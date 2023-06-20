use anyhow::Result;
use cashu::keyset::mint::KeySet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysetInfo {
    pub valid_from: u64,
    pub valid_to: Option<u64>,
    pub keyset: KeySet,
}

impl KeysetInfo {
    pub fn as_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LnMessage {
    PaymentReceived,
}
