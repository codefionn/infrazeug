//! Agentless lowering: ShellOp → remote argv / sftp steps (SOUL §3.3, §4.2).

use crate::op::{EnvVarSource, ShellOp};
use crate::source::FileSource;
use std::path::PathBuf;

/// Remote execution plan for agentless SSH.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lowered {
    Exec {
        argv: Vec<String>,
    },
    SftpWrite {
        path: PathBuf,
        content: Vec<u8>,
        mode: u32,
    },
    SftpRead {
        path: PathBuf,
    },
    Mkdir {
        path: PathBuf,
        mode: u32,
    },
    Seq {
        steps: Vec<Lowered>,
    },
}

pub fn lower(op: &ShellOp) -> Result<Lowered, String> {
    match op {
        ShellOp::Run { argv, cwd, env } => {
            if argv.is_empty() {
                return Err("empty argv".into());
            }
            let mut command = argv_to_remote_command(argv);
            if !env.is_empty() {
                let assignments = env
                    .iter()
                    .map(env_assignment)
                    .collect::<Result<Vec<_>, _>>()?
                    .join(" ");
                command = format!("{assignments} {command}");
            }
            if let Some(cwd) = cwd {
                command = format!(
                    "cd {} && {command}",
                    shell_escape(&cwd.display().to_string())
                );
            }
            Ok(Lowered::Exec {
                argv: vec![command],
            })
        }
        ShellOp::Seq { steps } => {
            let lowered: Result<Vec<_>, _> = steps.iter().map(lower).collect();
            Ok(Lowered::Seq { steps: lowered? })
        }
        ShellOp::ReadFile { path } => Ok(Lowered::SftpRead { path: path.clone() }),
        ShellOp::WriteFile {
            path,
            content,
            mode,
        } => {
            let content =
                crate::resolve::resolve_literal_file_source(content).map_err(|e| e.to_string())?;
            let crate::source::FileSource::Bytes(content) = content else {
                return Err(
                    "WriteFile capture/vault refs must be resolved before agentless lowering"
                        .into(),
                );
            };
            Ok(Lowered::SftpWrite {
                path: path.clone(),
                content: content.clone(),
                mode: *mode,
            })
        }
        ShellOp::VaultWrite { .. } => Err(
            "ShellOp::VaultWrite must be handled by the controller, not lowered to agentless SSH"
                .into(),
        ),
        ShellOp::VaultEnsurePasswordHash { .. } => Err(
            "ShellOp::VaultEnsurePasswordHash must be handled by the controller, not lowered to agentless SSH"
                .into(),
        ),
        ShellOp::EnsureDir { path, mode } => Ok(Lowered::Mkdir {
            path: path.clone(),
            mode: *mode,
        }),
        ShellOp::SyncDir { .. } => Err(
            "ShellOp::SyncDir is controller-side and cannot be lowered to remote argv/SFTP".into(),
        ),
        ShellOp::Poll { .. } => Err(
            "ShellOp::Poll must be handled by the scheduler, not lowered to agentless SSH".into(),
        ),
    }
}

/// Build a single remote `sh -c` argv for `Lowered::Exec` steps inside a sequence.
/// One remote shell command line for `ssh host -- <cmd>` (OpenSSH joins multiple argv with spaces).
pub fn argv_to_remote_command(argv: &[String]) -> String {
    argv.iter()
        .map(|a| shell_escape(a))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn lowered_exec_argv(lowered: &Lowered) -> Result<Vec<String>, String> {
    match lowered {
        Lowered::Exec { argv } => Ok(argv.clone()),
        Lowered::Seq { steps } => {
            let mut parts = Vec::new();
            for step in steps {
                match step {
                    Lowered::Exec { argv } => {
                        let [cmd] = argv.as_slice() else {
                            return Err("agentless seq exec must be a single remote command".into());
                        };
                        if !parts.is_empty() {
                            parts.push("&&".to_string());
                        }
                        parts.push(cmd.clone());
                    }
                    _ => return Err("agentless seq supports exec steps only".into()),
                }
            }
            Ok(vec![parts.join(" ")])
        }
        _ => Err("not an exec lowered op".into()),
    }
}

fn env_assignment(entry: &EnvVarSource) -> Result<String, String> {
    validate_env_name(&entry.name)?;
    let content =
        crate::resolve::resolve_literal_file_source(&entry.value).map_err(|e| e.to_string())?;
    let FileSource::Bytes(bytes) = content else {
        return Err("Run env capture/vault refs must be resolved before agentless lowering".into());
    };
    let value = String::from_utf8(bytes)
        .map_err(|e| format!("env `{}` value is not utf-8: {e}", entry.name))?;
    if value.contains('\0') {
        return Err(format!("env `{}` value contains NUL byte", entry.name));
    }
    Ok(format!("{}={}", entry.name, shell_escape(&value)))
}

fn validate_env_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains('=') || name.contains('\0') {
        return Err(format!("invalid env name `{name}`"));
    }
    Ok(())
}

pub fn shell_escape(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "./-_:".contains(c))
    {
        return s.to_string();
    }
    let mut out = String::from("'");
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::argv;

    #[test]
    fn run_lowers_to_single_remote_command() {
        let op = ShellOp::run(argv!["nginx", "-v"]);
        let Lowered::Exec { argv } = lower(&op).unwrap() else {
            panic!("expected exec");
        };
        assert_eq!(argv, vec!["nginx -v".to_string()]);
    }

    #[test]
    fn sudo_lowers_to_single_remote_command() {
        let op = ShellOp::run(argv!["sudo", "-n", "true"]);
        let Lowered::Exec { argv } = lower(&op).unwrap() else {
            panic!("expected exec");
        };
        assert_eq!(argv, vec!["sudo -n true".to_string()]);
        assert_eq!(lowered_exec_argv(&lower(&op).unwrap()).unwrap(), argv);
    }

    #[test]
    fn write_file_lowers_sftp() {
        let op = ShellOp::write_file_bytes("/tmp/x", b"hi", 0o644);
        assert!(matches!(lower(&op).unwrap(), Lowered::SftpWrite { .. }));
    }

    #[test]
    fn run_env_lowers_to_prefix_assignments() {
        let op = ShellOp::run_with_env(
            argv!["sh", "-c", "printf %s \"$PW\""],
            [("PW", crate::source::FileSource::bytes(b"a b"))],
        );
        let Lowered::Exec { argv } = lower(&op).unwrap() else {
            panic!("expected exec");
        };
        assert_eq!(argv, vec!["PW='a b' sh -c 'printf %s \"$PW\"'".to_string()]);
    }
}
