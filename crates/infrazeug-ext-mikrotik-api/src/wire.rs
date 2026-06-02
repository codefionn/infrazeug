//! MikroTik API wire encoding: length-prefixed words and sentences.

use crate::error::{MikrotikError, Result};
use std::collections::HashMap;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// A single API reply sentence (`!done`, `!re`, `!trap`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// Reply kind (`done`, `re`, `trap`, `fatal`, `empty`).
    pub kind: String,
    /// Attribute words (`=name=value`); keys omit the leading `=`.
    pub attrs: HashMap<String, String>,
    /// `.tag` value when the router echoed one.
    pub tag: Option<String>,
}

/// Build and send an API sentence (command + attributes + zero-length terminator).
#[derive(Debug, Clone, Default)]
pub struct Sentence {
    words: Vec<String>,
}

impl Sentence {
    /// Start a sentence with a command word (e.g. `/ip/address/print`).
    pub fn command(cmd: impl Into<String>) -> Self {
        let mut s = Self::default();
        s.words.push(cmd.into());
        s
    }

    /// Append `=name=value`.
    pub fn attr(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.words
            .push(format!("={}={}", name.as_ref(), value.as_ref()));
        self
    }

    /// Append `=name=value` when `value` is present.
    pub fn attr_opt(mut self, name: impl AsRef<str>, value: Option<&str>) -> Self {
        if let Some(v) = value {
            self.words.push(format!("={}={}", name.as_ref(), v));
        }
        self
    }

    /// Append a boolean attribute (`=name=yes` / `=name=no`).
    pub fn attr_bool(self, name: impl AsRef<str>, value: bool) -> Self {
        self.attr(name, if value { "yes" } else { "no" })
    }

    /// Append an API attribute (`.name=value`), e.g. `.tag=3` or `.proplist=a,b`.
    pub fn api_attr(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.words
            .push(format!(".{}={}", name.as_ref(), value.as_ref()));
        self
    }

    /// Shorthand for `.tag=N`.
    pub fn tag(self, tag: impl AsRef<str>) -> Self {
        self.api_attr("tag", tag.as_ref())
    }

    /// Shorthand for `.proplist=…` on print commands.
    pub fn proplist(self, props: &[&str]) -> Self {
        self.api_attr("proplist", props.join(","))
    }

    /// Append a print query word (`?name=value`). Order matters for RouterOS.
    pub fn query(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.words
            .push(format!("?{}={}", name.as_ref(), value.as_ref()));
        self
    }

    pub(crate) fn words(&self) -> &[String] {
        &self.words
    }
}

/// Encode a word (length prefix + content).
pub fn encode_word(buf: &mut Vec<u8>, content: &[u8]) -> Result<()> {
    let len = content.len();
    if len <= 0x7F {
        buf.push(len as u8);
    } else if len <= 0x3FFF {
        let v = (len as u16) | 0x8000;
        buf.extend_from_slice(&v.to_be_bytes());
    } else if len <= 0x1FFFFF {
        let v = (len as u32) | 0xC0_00_00;
        let b = v.to_be_bytes();
        buf.extend_from_slice(&b[1..]);
    } else if len <= 0x0FFF_FFFF {
        let v = (len as u32) | 0xE0_00_00_00;
        buf.extend_from_slice(&v.to_be_bytes());
    } else if len <= 0x7FFF_FFFF {
        buf.push(0xF0);
        buf.extend_from_slice(&(len as u32).to_be_bytes());
    } else {
        return Err(MikrotikError::Protocol(format!(
            "word length {len} exceeds supported maximum"
        )));
    }
    buf.extend_from_slice(content);
    Ok(())
}

/// Decode one length-prefixed word from `data`, returning the word and bytes consumed.
pub fn decode_word(data: &[u8]) -> Result<(&[u8], usize)> {
    if data.is_empty() {
        return Err(MikrotikError::Protocol("unexpected end of stream".into()));
    }
    let first = data[0];
    let (len, header_len): (usize, usize) = if first <= 0x7F {
        (first as usize, 1)
    } else if (first & 0xC0) == 0x80 {
        if data.len() < 2 {
            return Err(MikrotikError::Protocol("truncated 2-byte length".into()));
        }
        let v = u16::from_be_bytes([data[0], data[1]]) & 0x3FFF;
        (v as usize, 2)
    } else if (first & 0xE0) == 0xC0 {
        if data.len() < 3 {
            return Err(MikrotikError::Protocol("truncated 3-byte length".into()));
        }
        let v = u32::from_be_bytes([0, data[0], data[1], data[2]]) & 0x1F_FFFF;
        (v as usize, 3)
    } else if (first & 0xF0) == 0xE0 {
        if data.len() < 4 {
            return Err(MikrotikError::Protocol("truncated 4-byte length".into()));
        }
        let v = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) & 0x0FFF_FFFF;
        (v as usize, 4)
    } else if first == 0xF0 {
        if data.len() < 5 {
            return Err(MikrotikError::Protocol("truncated 5-byte length".into()));
        }
        let v = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;
        (v, 5)
    } else if first >= 0xF8 {
        return Err(MikrotikError::Protocol(format!(
            "reserved control byte 0x{first:02X}"
        )));
    } else {
        return Err(MikrotikError::Protocol(format!(
            "invalid length prefix 0x{first:02X}"
        )));
    };

    let total = header_len + len;
    if data.len() < total {
        return Err(MikrotikError::Protocol("truncated word payload".into()));
    }
    Ok((&data[header_len..total], total))
}

fn parse_word(word: &str) -> (Option<String>, Option<(String, String)>) {
    if let Some(rest) = word.strip_prefix('.') {
        if let Some((k, v)) = rest.split_once('=') {
            return (Some(k.to_string()), Some((format!(".{k}"), v.to_string())));
        }
        return (None, None);
    }
    if let Some(rest) = word.strip_prefix('=') {
        if let Some((k, v)) = rest.split_once('=') {
            return (None, Some((k.to_string(), v.to_string())));
        }
        if !rest.is_empty() {
            return (None, Some((rest.to_string(), String::new())));
        }
    } else if let Some(stripped) = word.strip_prefix('!') {
        return (None, Some(("!kind".into(), stripped.to_string())));
    } else if word.starts_with('?') {
        // Query words are outbound-only in this client.
        return (None, None);
    } else if !word.is_empty() && word.starts_with('/') {
        return (None, Some(("!cmd".into(), word.to_string())));
    }
    (None, None)
}

fn reply_from_words(words: &[String]) -> Reply {
    let mut kind = String::new();
    let mut attrs = HashMap::new();
    let mut tag = None;
    for w in words {
        let (api_tag, attr) = parse_word(w);
        if let Some(t) = api_tag {
            if t == "tag" {
                if let Some((_, ref v)) = attr {
                    tag = Some(v.clone());
                }
            }
        }
        if let Some((k, v)) = attr {
            if k == "!kind" {
                kind = v;
            } else if k.starts_with('.') {
                let key = k.trim_start_matches('.').to_string();
                if key == "tag" {
                    tag = Some(v);
                } else {
                    attrs.insert(key, v);
                }
            } else {
                attrs.insert(k, v);
            }
        }
    }
    Reply { kind, attrs, tag }
}

/// Send a sentence and read replies until `!done`, `!trap`, or `!fatal`.
pub async fn exchange<S>(stream: &mut S, sentence: &Sentence) -> Result<Vec<Reply>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut payload = Vec::new();
    for word in sentence.words() {
        encode_word(&mut payload, word.as_bytes())?;
    }
    encode_word(&mut payload, &[])?; // sentence terminator
    stream.write_all(&payload).await.map_err(transport_err)?;
    stream.flush().await.map_err(transport_err)?;

    let mut replies = Vec::new();
    let mut current_words = Vec::new();
    loop {
        let word = read_word(stream).await?;
        if word.is_empty() {
            if !current_words.is_empty() {
                replies.push(reply_from_words(&current_words));
                current_words.clear();
            }
            if let Some(last) = replies.last() {
                match last.kind.as_str() {
                    "trap" | "fatal" => break,
                    "done" | "empty" => break,
                    _ => {}
                }
            }
            continue;
        }
        current_words.push(String::from_utf8_lossy(&word).into_owned());
    }
    Ok(replies)
}

async fn read_word<S>(stream: &mut S) -> Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 1];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(transport_err)?;
    let first = len_buf[0];
    let header = if first <= 0x7F {
        vec![first]
    } else if (first & 0xC0) == 0x80 {
        let mut b = [0u8; 2];
        b[0] = first;
        stream
            .read_exact(&mut b[1..])
            .await
            .map_err(transport_err)?;
        b.to_vec()
    } else if (first & 0xE0) == 0xC0 {
        let mut b = [0u8; 3];
        b[0] = first;
        stream
            .read_exact(&mut b[1..])
            .await
            .map_err(transport_err)?;
        b.to_vec()
    } else if (first & 0xF0) == 0xE0 {
        let mut b = [0u8; 4];
        b[0] = first;
        stream
            .read_exact(&mut b[1..])
            .await
            .map_err(transport_err)?;
        b.to_vec()
    } else if first == 0xF0 {
        let mut b = [0u8; 5];
        b[0] = first;
        stream
            .read_exact(&mut b[1..])
            .await
            .map_err(transport_err)?;
        b.to_vec()
    } else if first >= 0xF8 {
        return Err(MikrotikError::Protocol(format!(
            "reserved control byte 0x{first:02X}"
        )));
    } else {
        return Err(MikrotikError::Protocol(format!(
            "invalid length prefix 0x{first:02X}"
        )));
    };

    let (content, _) = decode_word(&header)?;
    if content.is_empty() {
        return Ok(Vec::new());
    }
    let mut body = vec![0u8; content.len()];
    stream.read_exact(&mut body).await.map_err(transport_err)?;
    Ok(body)
}

fn transport_err(e: std::io::Error) -> MikrotikError {
    MikrotikError::Transport(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_short_word() {
        let mut buf = Vec::new();
        encode_word(&mut buf, b"abc").unwrap();
        assert_eq!(buf, [3, b'a', b'b', b'c']);
    }

    #[test]
    fn encode_zero_length_word() {
        let mut buf = Vec::new();
        encode_word(&mut buf, b"").unwrap();
        assert_eq!(buf, [0]);
    }

    #[test]
    fn encode_127_byte_word() {
        let data = vec![b'x'; 0x7F];
        let mut buf = Vec::new();
        encode_word(&mut buf, &data).unwrap();
        assert_eq!(buf[0], 0x7F);
        assert_eq!(buf.len(), 1 + 0x7F);
    }

    #[test]
    fn encode_128_byte_word() {
        let data = vec![b'y'; 0x80];
        let mut buf = Vec::new();
        encode_word(&mut buf, &data).unwrap();
        assert_eq!(buf[0], 0x80);
        assert_eq!(buf[1], 0x80);
        assert_eq!(buf.len(), 2 + 0x80);
    }

    #[test]
    fn roundtrip_words() {
        for len in [0, 1, 0x7F, 0x80, 0x3FFF, 0x4000] {
            let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            let mut encoded = Vec::new();
            encode_word(&mut encoded, &data).unwrap();
            let (decoded, consumed) = decode_word(&encoded).unwrap();
            assert_eq!(consumed, encoded.len());
            assert_eq!(decoded, data.as_slice(), "len={len}");
        }
    }

    #[test]
    fn reply_parses_re_sentence() {
        let words = vec![
            "!re".to_string(),
            "=.id=*1".to_string(),
            "=address=10.0.0.1/24".to_string(),
            "=interface=ether1".to_string(),
        ];
        let reply = reply_from_words(&words);
        assert_eq!(reply.kind, "re");
        assert_eq!(reply.attrs.get("id").map(String::as_str), Some("*1"));
        assert_eq!(
            reply.attrs.get("address").map(String::as_str),
            Some("10.0.0.1/24")
        );
    }

    #[test]
    fn sentence_builder_formats_attrs() {
        let s = Sentence::command("/ip/address/add")
            .attr("address", "10.0.0.1/24")
            .attr("interface", "ether1");
        assert_eq!(s.words()[0], "/ip/address/add");
        assert!(s.words()[1].starts_with("=address="));
    }
}
