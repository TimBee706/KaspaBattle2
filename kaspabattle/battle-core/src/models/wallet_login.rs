use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletLoginReq {
    pub kaspa_address: String,
    pub message: String,
    pub signature: String,
}
