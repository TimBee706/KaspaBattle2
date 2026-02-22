use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OracleJobStatus {
    Pending,
    Processing,
    Resolved,
    Failed,
    Cancelled,
}

impl OracleJobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OracleJobStatus::Pending => "Pending",
            OracleJobStatus::Processing => "Processing",
            OracleJobStatus::Resolved => "Resolved",
            OracleJobStatus::Failed => "Failed",
            OracleJobStatus::Cancelled => "Cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Pending" => Some(OracleJobStatus::Pending),
            "Processing" => Some(OracleJobStatus::Processing),
            "Resolved" => Some(OracleJobStatus::Resolved),
            "Failed" => Some(OracleJobStatus::Failed),
            "Cancelled" => Some(OracleJobStatus::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleJob {
    pub job_id: String,
    pub match_id: String,
    pub faceit_match_id: String,
    pub status: OracleJobStatus,
    pub reported_winner: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OracleCreateRequest {
    pub match_id: String,
    pub faceit_match_id: String,
}
