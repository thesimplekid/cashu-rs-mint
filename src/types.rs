use anyhow::Result;
use cashu_sdk::nuts::nut02::Id;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysetInfo {
    pub id: Id,
    pub valid_from: u64,
    pub valid_to: Option<u64>,
    pub secret: String,
    pub derivation_path: String,
    pub max_order: u8,
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
