//! UDS control channel for TUI attach (SOUL §6ter.1).

use crate::interactor::{Interaction, InteractionResp};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlMsg {
    Event(SchedEventWire),
    Prompt(Interaction),
    PromptResp(InteractionResp),
    Command(SchedCommandWire),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchedEventWire {
    NodeQueued {
        node: String,
        machine: String,
    },
    NodeStarted {
        node: String,
        machine: String,
    },
    NodeProgress {
        node: String,
        machine: String,
        message: String,
    },
    NodeFinished {
        node: String,
        machine: String,
        status: String,
        duration_ms: u64,
    },
    NodeCancelled {
        node: String,
        machine: String,
        reason: String,
    },
    PlanWarning {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchedCommandWire {
    CancelNode {
        node: String,
        machine: String,
        grace_ms: u64,
    },
    CancelMachine {
        machine: String,
    },
    PauseAll,
    ResumeAll,
    ReplayNode {
        node: String,
        machine: String,
    },
}

pub async fn serve_control(
    socket_path: &Path,
    on_command: Arc<Mutex<dyn Fn(SchedCommandWire) + Send + Sync>>,
) -> std::io::Result<()> {
    if socket_path.exists() {
        let _ = std::fs::remove_file(socket_path);
    }
    let listener = UnixListener::bind(socket_path)?;
    loop {
        let (mut stream, _) = listener.accept().await?;
        let handler = Arc::clone(&on_command);
        tokio::spawn(async move {
            loop {
                match read_msg(&mut stream).await {
                    Ok(ControlMsg::Command(cmd)) => handler.lock().await(cmd),
                    Ok(ControlMsg::PromptResp(_)) => {}
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });
    }
}

pub async fn send_event(stream: &mut UnixStream, ev: SchedEventWire) -> std::io::Result<()> {
    write_msg(stream, ControlMsg::Event(ev)).await
}

pub async fn read_prompt(stream: &mut UnixStream) -> std::io::Result<Interaction> {
    loop {
        if let ControlMsg::Prompt(p) = read_msg(stream).await? {
            return Ok(p);
        }
    }
}

pub async fn write_msg(stream: &mut UnixStream, msg: ControlMsg) -> std::io::Result<()> {
    let bytes = postcard::to_allocvec(&msg).map_err(std::io::Error::other)?;
    let len = (bytes.len() as u32).to_be_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&bytes).await?;
    Ok(())
}

pub async fn read_msg(stream: &mut UnixStream) -> std::io::Result<ControlMsg> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    postcard::from_bytes(&buf).map_err(std::io::Error::other)
}
