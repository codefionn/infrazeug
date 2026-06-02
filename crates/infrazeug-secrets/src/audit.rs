use std::io::Write;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct AuditEntry {
    pub who: String,
    pub op: String,
    pub file_id: String,
    pub data_key_id: String,
    pub host: String,
    pub success: bool,
}

pub fn append_audit(run_audit_dir: &Path, entry: &AuditEntry) -> std::io::Result<()> {
    std::fs::create_dir_all(run_audit_dir)?;
    let date = chrono_lite_date();
    let path = run_audit_dir.join(format!("{date}.log"));
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(
        f,
        "{} who={} op={} file={} key={} host={} ok={}",
        timestamp(),
        entry.who,
        entry.op,
        entry.file_id,
        entry.data_key_id,
        entry.host,
        entry.success
    )?;
    Ok(())
}

fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn chrono_lite_date() -> String {
    // Avoid chrono dependency: UTC date from epoch days
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0);
    // 1970-01-01 + days (approximate, good enough for audit filenames)
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524) / 365;
    let y = (yoe as i32) + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y, m as u32, d as u32)
}
