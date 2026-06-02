use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentDigest(pub [u8; 32]);

impl ContentDigest {
    pub fn hash_bytes(data: &[u8]) -> Self {
        let mut h = Sha256::new();
        h.update(data);
        Self(h.finalize().into())
    }

    pub fn hash_json<T: serde::Serialize>(value: &T) -> Result<Self, serde_json::Error> {
        let bytes = serde_json::to_vec(value)?;
        Ok(Self::hash_bytes(&bytes))
    }

    pub fn short(&self) -> String {
        hex::encode(&self.0[..8])
    }
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sha256:{}", hex::encode(self.0))
    }
}
