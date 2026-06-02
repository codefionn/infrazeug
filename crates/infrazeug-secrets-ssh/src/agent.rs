use infrazeug_secrets::{Result, SecretsError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

const SSH_AGENTC_REQUEST_IDENTITIES: u8 = 11;
const SSH_AGENT_IDENTITIES_ANSWER: u8 = 12;
const SSH_AGENTC_SIGN_REQUEST: u8 = 13;
const SSH_AGENT_SIGN_RESPONSE: u8 = 14;
const MAX_AGENT_FRAME: usize = 16 * 1024 * 1024;

pub async fn ssh_agent_sign(comment: &str, data: &[u8]) -> Result<Vec<u8>> {
    let sock = std::env::var("SSH_AUTH_SOCK")
        .map_err(|_| SecretsError::Provider("SSH_AUTH_SOCK not set".into()))?;
    let mut stream = UnixStream::connect(sock)
        .await
        .map_err(|e| SecretsError::Provider(e.to_string()))?;

    let req = vec![SSH_AGENTC_REQUEST_IDENTITIES];
    write_frame(&mut stream, &req).await?;
    let resp = read_frame(&mut stream).await?;
    if resp.first() != Some(&SSH_AGENT_IDENTITIES_ANSWER) {
        return Err(SecretsError::Provider("identities answer".into()));
    }
    let blob = &resp[1..];
    let mut pubkey: Option<Vec<u8>> = None;
    let mut i = 0usize;
    if blob.len() < 4 {
        return Err(SecretsError::Provider("empty identities".into()));
    }
    let nkeys = u32::from_be_bytes(blob[0..4].try_into().unwrap()) as usize;
    i += 4;
    for _ in 0..nkeys {
        if i + 4 > blob.len() {
            break;
        }
        let klen = u32::from_be_bytes(blob[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        if i + klen > blob.len() {
            return Err(SecretsError::Provider("truncated identity key".into()));
        }
        let key = &blob[i..i + klen];
        i += klen;
        if i + 4 > blob.len() {
            break;
        }
        let clen = u32::from_be_bytes(blob[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        if i + clen > blob.len() {
            return Err(SecretsError::Provider("truncated identity comment".into()));
        }
        let c = std::str::from_utf8(&blob[i..i + clen])
            .unwrap_or_default()
            .to_string();
        i += clen;
        if c == comment {
            pubkey = Some(key.to_vec());
            break;
        }
    }
    let pubkey = pubkey
        .ok_or_else(|| SecretsError::Provider(format!("ssh-agent key {comment:?} not found")))?;

    let mut body = Vec::new();
    body.extend_from_slice(&(pubkey.len() as u32).to_be_bytes());
    body.extend_from_slice(&pubkey);
    body.extend_from_slice(&(data.len() as u32).to_be_bytes());
    body.extend_from_slice(data);
    body.push(0); // SSH_AGENT_RSA_SHA2_256 / flags for ed25519 default

    let mut req = vec![SSH_AGENTC_SIGN_REQUEST];
    req.extend_from_slice(&body);
    write_frame(&mut stream, &req).await?;
    let resp = read_frame(&mut stream).await?;
    if resp.first() != Some(&SSH_AGENT_SIGN_RESPONSE) {
        return Err(SecretsError::Provider("sign response".into()));
    }
    if resp.len() < 5 {
        return Err(SecretsError::Provider("short signature".into()));
    }
    let slen = u32::from_be_bytes(resp[1..5].try_into().unwrap()) as usize;
    if 5 + slen > resp.len() {
        return Err(SecretsError::Provider("truncated signature".into()));
    }
    Ok(resp[5..5 + slen].to_vec())
}

async fn write_frame(stream: &mut UnixStream, payload: &[u8]) -> Result<()> {
    let len = (payload.len() as u32).to_be_bytes();
    stream
        .write_all(&len)
        .await
        .map_err(|e| SecretsError::Provider(e.to_string()))?;
    stream
        .write_all(payload)
        .await
        .map_err(|e| SecretsError::Provider(e.to_string()))?;
    Ok(())
}

async fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| SecretsError::Provider(e.to_string()))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_AGENT_FRAME {
        return Err(SecretsError::Provider("ssh-agent frame too large".into()));
    }
    let mut buf = vec![0u8; len];
    stream
        .read_exact(&mut buf)
        .await
        .map_err(|e| SecretsError::Provider(e.to_string()))?;
    Ok(buf)
}
