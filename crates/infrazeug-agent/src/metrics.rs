//! Local host resource sampling for the push-mode agent's out-of-band metrics.
//!
//! Linux-first and dependency-free: CPU and memory are read from `/proc`; disk
//! usage comes from parsing `df -kP /` (POSIX output) rather than binding
//! `statvfs(2)`, so there is no extra crate and no per-arch `unsafe` syscall.
//! CPU/memory failure (e.g. no `/proc`) skips the sample; disk is best-effort
//! and reports `0/0` if `df` is unavailable so CPU/memory still flow.

use infrazeug_rpc::AgentMetrics;
use std::io::{self, ErrorKind};

/// Holds the previous `/proc/stat` CPU totals so each sample reports busy
/// fraction over the interval since the last call rather than since boot.
#[derive(Default)]
pub struct CpuSampler {
    prev: Option<(u64, u64)>, // (idle, total)
}

impl CpuSampler {
    pub fn new() -> Self {
        Self { prev: None }
    }

    /// Busy CPU percentage (0.0–100.0) over the window since the last sample.
    /// The first call has no baseline and reports 0.0.
    fn sample(&mut self) -> io::Result<f32> {
        let (idle, total) = read_cpu_totals()?;
        let pct = match self.prev {
            Some((prev_idle, prev_total)) => {
                let d_total = total.saturating_sub(prev_total);
                let d_idle = idle.saturating_sub(prev_idle);
                if d_total == 0 {
                    0.0
                } else {
                    let busy = d_total.saturating_sub(d_idle) as f32;
                    (busy / d_total as f32) * 100.0
                }
            }
            None => 0.0,
        };
        self.prev = Some((idle, total));
        Ok(pct.clamp(0.0, 100.0))
    }
}

/// Collect one resource sample. CPU is relative to the previous `sampler` call;
/// memory and disk are absolute. Disk is best-effort: it reports `0/0` rather
/// than failing the whole sample when `df` is unavailable.
pub async fn collect(sampler: &mut CpuSampler) -> io::Result<AgentMetrics> {
    let cpu_pct = sampler.sample()?;
    let (mem_used, mem_total) = read_mem()?;
    let (disk_used, disk_total) = read_disk("/").await.unwrap_or((0, 0));
    Ok(AgentMetrics {
        cpu_pct,
        mem_used,
        mem_total,
        disk_used,
        disk_total,
    })
}

/// Parse the aggregate `cpu` line of `/proc/stat` into (idle, total) jiffies.
/// Fields: user nice system idle iowait irq softirq steal guest guest_nice.
/// Idle counts `idle + iowait`; total is the sum of all present fields.
fn read_cpu_totals() -> io::Result<(u64, u64)> {
    let stat = std::fs::read_to_string("/proc/stat")?;
    let line = stat
        .lines()
        .next()
        .filter(|l| l.starts_with("cpu "))
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "no cpu line in /proc/stat"))?;
    let fields: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|f| f.parse().ok())
        .collect();
    if fields.len() < 4 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "short /proc/stat cpu line",
        ));
    }
    let total: u64 = fields.iter().sum();
    let idle = fields[3] + fields.get(4).copied().unwrap_or(0); // idle + iowait
    Ok((idle, total))
}

/// Read (used, total) memory in bytes from `/proc/meminfo`. Used is
/// `MemTotal - MemAvailable` (what the kernel reports as actually free for new
/// work), matching what tools like `free` show.
fn read_mem() -> io::Result<(u64, u64)> {
    let info = std::fs::read_to_string("/proc/meminfo")?;
    let mut total_kb = None;
    let mut avail_kb = None;
    for line in info.lines() {
        if let Some(v) = meminfo_kb(line, "MemTotal:") {
            total_kb = Some(v);
        } else if let Some(v) = meminfo_kb(line, "MemAvailable:") {
            avail_kb = Some(v);
        }
        if total_kb.is_some() && avail_kb.is_some() {
            break;
        }
    }
    let total = total_kb
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "no MemTotal in /proc/meminfo"))?;
    let avail = avail_kb.unwrap_or(0);
    let used = total.saturating_sub(avail);
    Ok((used * 1024, total * 1024))
}

/// Parse `"<key> <value> kB"` to value (kB) when the line matches `key`.
fn meminfo_kb(line: &str, key: &str) -> Option<u64> {
    line.strip_prefix(key)?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// Read (used, total) bytes for the filesystem containing `path` by running
/// `df -kP <path>`. `-P` forces single-line-per-filesystem POSIX output and
/// `-k` forces 1024-byte block units, so the columns are stable:
/// `Filesystem 1024-blocks Used Available Capacity Mounted-on`.
async fn read_disk(path: &str) -> io::Result<(u64, u64)> {
    let out = tokio::process::Command::new("df")
        .args(["-kP", path])
        .output()
        .await?;
    if !out.status.success() {
        return Err(io::Error::other("df exited non-zero"));
    }
    parse_df(&String::from_utf8_lossy(&out.stdout))
}

/// Parse `df -kP` output into (used, total) bytes from its data row. Block
/// counts are in 1024-byte units (`-k`).
fn parse_df(text: &str) -> io::Result<(u64, u64)> {
    let row = text
        .lines()
        .nth(1) // skip the header; -P guarantees one data line per filesystem
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "no df data row"))?;
    let fields: Vec<&str> = row.split_whitespace().collect();
    // Filesystem, 1024-blocks, Used, Available, Capacity, Mounted on
    if fields.len() < 4 {
        return Err(io::Error::new(ErrorKind::InvalidData, "short df row"));
    }
    let blocks: u64 = fields[1]
        .parse()
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, "bad df total"))?;
    let used: u64 = fields[2]
        .parse()
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, "bad df used"))?;
    Ok((used * 1024, blocks * 1024))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_df_extracts_used_and_total_bytes() {
        let sample = "Filesystem     1024-blocks     Used Available Capacity Mounted on\n\
                      /dev/sda1         51474044 12345678  36805590      26% /\n";
        let (used, total) = parse_df(sample).unwrap();
        assert_eq!(total, 51474044 * 1024);
        assert_eq!(used, 12345678 * 1024);
    }

    #[test]
    fn parse_df_rejects_header_only() {
        let sample = "Filesystem 1024-blocks Used Available Capacity Mounted on\n";
        assert!(parse_df(sample).is_err());
    }
}
