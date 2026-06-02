use crate::error::{PullError, Result};
use bytes::Bytes;
use infrazeug_secrets::backend::Backend;
use std::sync::Arc;
use uuid::Uuid;

pub struct PlanStore {
    backend: Arc<dyn Backend>,
}

impl PlanStore {
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self { backend }
    }

    pub fn bootstrap_key(machine: Uuid) -> String {
        format!("bootstrap/{machine}.toml")
    }

    pub fn sealed_plan_key(machine: Uuid) -> String {
        format!("plans/{machine}.plan.sealed")
    }

    pub fn machine_pubkey_key(machine: Uuid) -> String {
        format!("machines/{machine}.pub")
    }

    pub fn tombstone_key(machine: Uuid) -> String {
        format!("tombstones/{machine}")
    }

    pub fn agent_key(digest: &str, triple: &str) -> String {
        format!("agents/{digest}/{triple}/infrazeug-agent")
    }

    pub fn agent_sig_key(digest: &str) -> String {
        format!("agents/{digest}.sig")
    }

    pub async fn put_bootstrap(&self, machine: Uuid, toml: &[u8]) -> Result<()> {
        self.backend
            .put(
                &Self::bootstrap_key(machine),
                Bytes::copy_from_slice(toml),
                None,
            )
            .await
            .map_err(|e| PullError::Store(e.to_string()))?;
        Ok(())
    }

    pub async fn get_sealed_plan(&self, machine: Uuid) -> Result<Option<Vec<u8>>> {
        match self
            .backend
            .get(&Self::sealed_plan_key(machine))
            .await
            .map_err(|e| PullError::Store(e.to_string()))?
        {
            Some((bytes, _)) => Ok(Some(bytes.to_vec())),
            None => Ok(None),
        }
    }

    pub async fn put_sealed_plan(&self, machine: Uuid, blob: &[u8]) -> Result<()> {
        self.backend
            .put(
                &Self::sealed_plan_key(machine),
                Bytes::copy_from_slice(blob),
                None,
            )
            .await
            .map_err(|e| PullError::Store(e.to_string()))?;
        Ok(())
    }

    pub async fn put_machine_pubkey(&self, machine: Uuid, pubkey: &[u8; 32]) -> Result<()> {
        self.backend
            .put(
                &Self::machine_pubkey_key(machine),
                Bytes::copy_from_slice(pubkey),
                None,
            )
            .await
            .map_err(|e| PullError::Store(e.to_string()))?;
        Ok(())
    }

    pub async fn get_machine_pubkey(&self, machine: Uuid) -> Result<[u8; 32]> {
        let (bytes, _) = self
            .backend
            .get(&Self::machine_pubkey_key(machine))
            .await
            .map_err(|e| PullError::Store(e.to_string()))?
            .ok_or_else(|| PullError::Store("machine pubkey not registered".into()))?;
        if bytes.len() != 32 {
            return Err(PullError::Store("pubkey must be 32 bytes".into()));
        }
        let mut pk = [0u8; 32];
        pk.copy_from_slice(&bytes);
        Ok(pk)
    }

    pub async fn is_revoked(&self, machine: Uuid) -> Result<bool> {
        Ok(self
            .backend
            .get(&Self::tombstone_key(machine))
            .await
            .map_err(|e| PullError::Store(e.to_string()))?
            .is_some())
    }

    pub async fn put_tombstone(&self, machine: Uuid, body: &[u8]) -> Result<()> {
        self.backend
            .put(
                &Self::tombstone_key(machine),
                Bytes::copy_from_slice(body),
                None,
            )
            .await
            .map_err(|e| PullError::Store(e.to_string()))?;
        Ok(())
    }

    pub async fn get_agent_sig(&self, digest: &str) -> Result<Option<Vec<u8>>> {
        match self
            .backend
            .get(&Self::agent_sig_key(digest))
            .await
            .map_err(|e| PullError::Store(e.to_string()))?
        {
            Some((b, _)) => Ok(Some(b.to_vec())),
            None => Ok(None),
        }
    }
}
