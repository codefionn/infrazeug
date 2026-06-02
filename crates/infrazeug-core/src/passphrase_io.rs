//! Hidden passphrase prompts for interactive apply and CLI.

use crate::error::{CoreError, Result};
use std::io::{self, BufRead, IsTerminal, Write};

/// Read a passphrase: hidden prompt on a TTY when possible, else one line from piped stdin.
pub fn read_passphrase_prompt(prompt: &str) -> Result<String> {
    if io::stdin().is_terminal() {
        return read_passphrase_tty(prompt);
    }
    // Some runners mark stdin non-TTY even on an interactive console; try /dev/tty first.
    if let Ok(secret) = read_passphrase_tty(prompt) {
        return Ok(secret);
    }
    read_passphrase_piped()
}

fn read_passphrase_tty(prompt: &str) -> Result<String> {
    eprint!("{prompt}");
    io::stderr().flush().ok();
    let secret = rpassword::read_password().map_err(|e| CoreError::other(e.to_string()))?;
    if secret.is_empty() {
        return Err(CoreError::other("empty passphrase"));
    }
    Ok(secret)
}

fn read_passphrase_piped() -> Result<String> {
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(CoreError::from)?;
    let line = line.trim_end_matches(['\r', '\n']).to_string();
    if line.is_empty() {
        return Err(CoreError::other("empty passphrase on stdin"));
    }
    Ok(line)
}
