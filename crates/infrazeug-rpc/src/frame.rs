use thiserror::Error;

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("postcard: {0}")]
    Postcard(#[from] postcard::Error),
    #[error("message too large")]
    TooLarge,
    #[error("unexpected eof")]
    Eof,
}

/// Encode `uvarint(len) || postcard(payload)`.
///
/// Wire format used by the RPC microarchitecture: the controller writes
/// framed `RpcRequest`s to the agent's stdin; the agent writes framed
/// `RpcResponse`s to stdout. Each frame is a length-prefix (unsigned
/// varint, LE, 7 bits/byte, MSB=continuation) followed by a postcard-
/// serialized payload.
pub fn encode<T: serde::Serialize>(msg: &T) -> Result<Vec<u8>, FrameError> {
    let body = postcard::to_allocvec(msg)?;
    let mut out = encode_uvarint(body.len() as u64);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Decode one framed message from a buffer; returns (message, bytes consumed).
pub fn decode_one<T: serde::de::DeserializeOwned>(buf: &[u8]) -> Result<(T, usize), FrameError> {
    let (len, hdr) = decode_uvarint(buf).ok_or(FrameError::Eof)?;
    let len = len as usize;
    let start = hdr;
    let end = start.checked_add(len).ok_or(FrameError::TooLarge)?;
    if end > buf.len() {
        return Err(FrameError::Eof);
    }
    let msg: T = postcard::from_bytes(&buf[start..end])?;
    Ok((msg, end))
}

fn encode_uvarint(mut n: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (n & 0x7f) as u8;
        n >>= 7;
        if n != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if n == 0 {
            break;
        }
    }
    out
}

fn decode_uvarint(buf: &[u8]) -> Option<(u64, usize)> {
    let mut n = 0u64;
    let mut shift = 0;
    for (i, &byte) in buf.iter().enumerate() {
        n |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some((n, i + 1));
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Ping {
        n: u32,
    }

    #[test]
    fn roundtrip() {
        let bytes = encode(&Ping { n: 42 }).unwrap();
        let (msg, n) = decode_one::<Ping>(&bytes).unwrap();
        assert_eq!(msg, Ping { n: 42 });
        assert_eq!(n, bytes.len());
    }

    #[test]
    fn decode_partial_buffer_returns_eof() {
        let bytes = encode(&Ping { n: 1 }).unwrap();
        let err = decode_one::<Ping>(&bytes[..bytes.len() - 1]).unwrap_err();
        assert!(matches!(err, FrameError::Eof));
    }

    #[test]
    fn decode_empty_buffer_returns_eof() {
        let err = decode_one::<Ping>(&[]).unwrap_err();
        assert!(matches!(err, FrameError::Eof));
    }

    #[test]
    fn uvarint_large_length() {
        let mut buf = encode_uvarint(10_000);
        buf.extend_from_slice(&[0u8; 8]);
        let err = decode_one::<Ping>(&buf).unwrap_err();
        assert!(matches!(err, FrameError::Eof));
    }

    // Regression: a `Run` op with empty `env` must survive a postcard round-trip.
    // `skip_serializing_if` on the field omits it from the (non-self-describing)
    // wire, and the deserializer then reads off the end — the bug that broke
    // every `connect/<host>` node with `postcard: Hit the end of buffer`.
    #[test]
    fn execute_shell_empty_env_roundtrips() {
        use crate::messages::RpcRequest;
        use infrazeug_shell::ShellOp;
        let req = RpcRequest::ExecuteShell {
            op: ShellOp::run(vec!["echo".into(), "$HOME".into()]),
        };
        let bytes = encode(&req).unwrap();
        let (decoded, n) = decode_one::<RpcRequest>(&bytes).unwrap();
        assert_eq!(decoded, req);
        assert_eq!(n, bytes.len());
    }

    // Guard against the postcard `skip_serializing_if` footgun across the whole
    // wire surface: every default-valued field on a type reachable from an
    // `RpcRequest`/`RpcResponse` must survive a postcard frame round-trip.
    // `skip_serializing_if` omits such fields from the (non-self-describing) wire
    // and the deserializer then reads off the end / misreads the next field.
    #[test]
    fn wire_default_fields_roundtrip() {
        use crate::messages::{RpcRequest, RpcResponse};
        use infrazeug_native::{NativeResult, NativeStatus};
        use infrazeug_shell::source::{CaptureRef, FileSource};
        use infrazeug_shell::{FileSourceTransform, PasswordHashSpec, ShellOp};

        fn rt_req(req: &RpcRequest) -> Result<(), String> {
            let bytes = encode(req).map_err(|e| e.to_string())?;
            match decode_one::<RpcRequest>(&bytes) {
                Ok((d, _)) if &d == req => Ok(()),
                Ok(_) => Err("round-trip mismatch".into()),
                Err(e) => Err(e.to_string()),
            }
        }
        fn rt_resp(resp: &RpcResponse) -> Result<(), String> {
            let bytes = encode(resp).map_err(|e| e.to_string())?;
            match decode_one::<RpcResponse>(&bytes) {
                Ok((d, _)) if &d == resp => Ok(()),
                Ok(_) => Err("round-trip mismatch".into()),
                Err(e) => Err(e.to_string()),
            }
        }
        let write = |content: FileSource| RpcRequest::ExecuteShell {
            op: ShellOp::WriteFile {
                path: "/tmp/x".into(),
                content,
                mode: 0o600,
            },
        };

        let mut broken = Vec::new();
        let req_cases: Vec<(&str, RpcRequest)> = vec![
            (
                "Run{env:[]}",
                RpcRequest::ExecuteShell {
                    op: ShellOp::run(vec!["echo".into()]),
                },
            ),
            (
                "FileSource::Capture{machine:None}",
                write(FileSource::Capture(CaptureRef {
                    node: uuid::Uuid::nil(),
                    machine: None,
                })),
            ),
            (
                "FileSource::Vault{field:None}",
                RpcRequest::ExecuteShell {
                    op: ShellOp::WriteFile {
                        path: "/tmp/x".into(),
                        content: FileSource::Vault {
                            file: "f".into(),
                            field: None,
                        },
                        mode: 0o600,
                    },
                },
            ),
            (
                "Transform::RegexInclude{capture:None}",
                RpcRequest::ExecuteShell {
                    op: ShellOp::WriteFile {
                        path: "/tmp/x".into(),
                        content: FileSource::Transform {
                            source: Box::new(FileSource::Bytes(vec![1, 2, 3])),
                            transforms: vec![FileSourceTransform::RegexInclude {
                                pattern: "x".into(),
                                capture: None,
                            }],
                        },
                        mode: 0o600,
                    },
                },
            ),
            (
                "Transform::JsonPointer{optional:false}",
                RpcRequest::ExecuteShell {
                    op: ShellOp::WriteFile {
                        path: "/tmp/x".into(),
                        content: FileSource::Transform {
                            source: Box::new(FileSource::Bytes(vec![1, 2, 3])),
                            transforms: vec![FileSourceTransform::JsonPointer {
                                path: "/a".into(),
                                optional: false,
                            }],
                        },
                        mode: 0o600,
                    },
                },
            ),
            (
                "PasswordHashSpec{output_len:None}",
                RpcRequest::ExecuteShell {
                    op: ShellOp::VaultEnsurePasswordHash {
                        data_key_id: "k".into(),
                        file: "f".into(),
                        password_field: "p".into(),
                        hash_field: "h".into(),
                        password: infrazeug_shell::RandomPasswordSpec::new(16),
                        hash: PasswordHashSpec::argon2id(),
                    },
                },
            ),
        ];
        for (name, req) in &req_cases {
            if let Err(e) = rt_req(req) {
                broken.push(format!("{name}: {e}"));
            }
        }

        let resp_cases: Vec<(&str, RpcResponse)> = vec![
            (
                "NativeResult{message:None,output:None,capture:None}",
                RpcResponse::NativeResult(NativeResult {
                    status: NativeStatus::Unchanged,
                    message: None,
                    output: None,
                    capture: None,
                }),
            ),
            (
                "NativeResult::changed",
                RpcResponse::NativeResult(NativeResult::changed("did a thing")),
            ),
        ];
        for (name, resp) in &resp_cases {
            if let Err(e) = rt_resp(resp) {
                broken.push(format!("{name}: {e}"));
            }
        }

        assert!(
            broken.is_empty(),
            "broken postcard round-trips: {broken:#?}"
        );
    }
}
