use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub stable_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplyPreview {
    pub status: &'static str,
    pub token: String,
    pub channel: &'static str,
    pub conversation: String,
    pub message: String,
    pub expires_at: i64,
    pub requires_user_confirmation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub version: u8,
    pub channel: String,
    pub conversation_name: String,
    pub conversation_id_sha256: String,
    pub message_sha256: String,
    pub message_length: usize,
    pub created_at: i64,
    pub expires_at: i64,
}
