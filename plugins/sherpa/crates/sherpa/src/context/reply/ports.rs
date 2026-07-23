use anyhow::Result;

use super::{ApprovalRecord, Conversation};

pub trait ConversationGateway {
    fn resolve_exact(&self, display_name: &str) -> Result<Conversation>;
    fn dispatch_text(&self, conversation: &Conversation, message: &str) -> Result<()>;
}

pub trait ApprovalRepository {
    fn remove_expired(&self, now: i64) -> Result<()>;
    fn save(&self, token: &str, record: &ApprovalRecord) -> Result<()>;
    fn load(&self, token: &str) -> Result<ApprovalRecord>;
    fn delete(&self, token: &str) -> Result<bool>;
}

pub trait Clock {
    fn now(&self) -> i64;
}

pub trait TokenGenerator {
    fn generate(&self) -> Result<String>;
}
