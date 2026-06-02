//! `like` twin helpers (SOUL §5.4).

use crate::spec::LikeConfig;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct LikeVars(pub BTreeMap<String, String>);

pub fn validate_like(like: &LikeConfig) -> crate::error::Result<()> {
    if !crate::spec::is_emulated_kind(&like.kind) {
        return Err(crate::error::EmulateError::LikeNotEmulated);
    }
    Ok(())
}
