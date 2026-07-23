use anyhow::{Result, bail};
use sha2::{Digest, Sha256};

use super::{
    ApprovalRecord, ApprovalRepository, Clock, ConversationGateway, ReplyPreview, TokenGenerator,
};

pub const DEFAULT_TTL_SECONDS: u64 = 900;
pub const MIN_TTL_SECONDS: u64 = 60;
pub const MAX_TTL_SECONDS: u64 = 1_800;
pub const MAX_MESSAGE_LENGTH: usize = 2_000;

pub struct ReplyService<'a> {
    gateway: &'a dyn ConversationGateway,
    approvals: &'a dyn ApprovalRepository,
    clock: &'a dyn Clock,
    tokens: &'a dyn TokenGenerator,
}

impl<'a> ReplyService<'a> {
    pub fn new(
        gateway: &'a dyn ConversationGateway,
        approvals: &'a dyn ApprovalRepository,
        clock: &'a dyn Clock,
        tokens: &'a dyn TokenGenerator,
    ) -> Self {
        Self {
            gateway,
            approvals,
            clock,
            tokens,
        }
    }

    pub fn prepare(
        &self,
        conversation_name: &str,
        message: String,
        ttl_seconds: u64,
    ) -> Result<ReplyPreview> {
        validate_message(&message)?;
        if !(MIN_TTL_SECONDS..=MAX_TTL_SECONDS).contains(&ttl_seconds) {
            bail!(
                "approval lifetime must be between {MIN_TTL_SECONDS} and {MAX_TTL_SECONDS} seconds"
            )
        }

        let conversation = self.gateway.resolve_exact(conversation_name)?;
        let now = self.clock.now();
        self.approvals.remove_expired(now)?;
        let token = self.tokens.generate()?;
        validate_token(&token)?;
        let expires_at = now + i64::try_from(ttl_seconds)?;
        self.approvals.save(
            &token,
            &ApprovalRecord {
                version: 1,
                channel: "kakaotalk".to_owned(),
                conversation_name: conversation.display_name.clone(),
                conversation_id_sha256: digest(&conversation.stable_id),
                message_sha256: digest(&message),
                message_length: message.chars().count(),
                created_at: now,
                expires_at,
            },
        )?;

        Ok(ReplyPreview {
            status: "preview",
            token,
            channel: "kakaotalk",
            conversation: conversation.display_name,
            message,
            expires_at,
            requires_user_confirmation: true,
        })
    }

    pub fn confirm(&self, token: &str, message: &str) -> Result<()> {
        validate_token(token)?;
        validate_message(message)?;
        let record = self.approvals.load(token)?;
        if record.version != 1 || record.channel != "kakaotalk" {
            bail!("approval record has an unsupported format")
        }
        if record.expires_at <= self.clock.now() {
            self.approvals.delete(token)?;
            bail!("approval token has expired")
        }
        if digest(message) != record.message_sha256 {
            bail!("reply text differs from the confirmed preview")
        }
        if message.chars().count() != record.message_length {
            bail!("reply text length differs from the confirmed preview")
        }

        let conversation = self.gateway.resolve_exact(&record.conversation_name)?;
        if digest(&conversation.stable_id) != record.conversation_id_sha256 {
            bail!("resolved conversation changed after preview")
        }

        self.gateway.dispatch_text(&conversation, message)?;
        self.approvals.delete(token)?;
        Ok(())
    }

    pub fn cancel(&self, token: &str) -> Result<bool> {
        validate_token(token)?;
        self.approvals.delete(token)
    }
}

pub fn validate_message(message: &str) -> Result<()> {
    if message.trim().is_empty() {
        bail!("reply message must be provided on standard input")
    }
    if message.contains('\0') {
        bail!("reply message must not contain NUL bytes")
    }
    if message.chars().count() > MAX_MESSAGE_LENGTH {
        bail!("reply message exceeds the {MAX_MESSAGE_LENGTH}-character limit")
    }
    Ok(())
}

pub fn validate_token(token: &str) -> Result<()> {
    if token.len() != 32
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("invalid approval token")
    }
    Ok(())
}

fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap};

    use anyhow::{Result, bail};

    use super::*;
    use crate::context::reply::{Conversation, ReplyPreview};

    struct FakeGateway {
        conversation: RefCell<Conversation>,
        dispatched: RefCell<Vec<String>>,
    }

    impl ConversationGateway for FakeGateway {
        fn resolve_exact(&self, display_name: &str) -> Result<Conversation> {
            let conversation = self.conversation.borrow().clone();
            if conversation.display_name != display_name {
                bail!("conversation missing")
            }
            Ok(conversation)
        }

        fn dispatch_text(&self, _conversation: &Conversation, message: &str) -> Result<()> {
            self.dispatched.borrow_mut().push(message.to_owned());
            Ok(())
        }
    }

    #[derive(Default)]
    struct MemoryApprovals(RefCell<HashMap<String, ApprovalRecord>>);

    impl ApprovalRepository for MemoryApprovals {
        fn remove_expired(&self, now: i64) -> Result<()> {
            self.0
                .borrow_mut()
                .retain(|_, record| record.expires_at > now);
            Ok(())
        }

        fn save(&self, token: &str, record: &ApprovalRecord) -> Result<()> {
            if self
                .0
                .borrow_mut()
                .insert(token.to_owned(), record.clone())
                .is_some()
            {
                bail!("token collision")
            }
            Ok(())
        }

        fn load(&self, token: &str) -> Result<ApprovalRecord> {
            self.0
                .borrow()
                .get(token)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("approval missing"))
        }

        fn delete(&self, token: &str) -> Result<bool> {
            Ok(self.0.borrow_mut().remove(token).is_some())
        }
    }

    struct FixedClock(i64);

    impl Clock for FixedClock {
        fn now(&self) -> i64 {
            self.0
        }
    }

    struct FixedToken;

    impl TokenGenerator for FixedToken {
        fn generate(&self) -> Result<String> {
            Ok("0123456789abcdef0123456789abcdef".to_owned())
        }
    }

    fn service<'a>(
        gateway: &'a FakeGateway,
        approvals: &'a MemoryApprovals,
        clock: &'a FixedClock,
        token: &'a FixedToken,
    ) -> ReplyService<'a> {
        ReplyService::new(gateway, approvals, clock, token)
    }

    fn gateway() -> FakeGateway {
        FakeGateway {
            conversation: RefCell::new(Conversation {
                stable_id: "stable-1".to_owned(),
                display_name: "Example".to_owned(),
            }),
            dispatched: RefCell::new(Vec::new()),
        }
    }

    fn prepare(
        gateway: &FakeGateway,
        approvals: &MemoryApprovals,
        clock: &FixedClock,
        token: &FixedToken,
    ) -> ReplyPreview {
        service(gateway, approvals, clock, token)
            .prepare(
                "Example",
                "Confirmed response".to_owned(),
                DEFAULT_TTL_SECONDS,
            )
            .unwrap()
    }

    #[test]
    fn confirmation_is_bound_to_message_and_conversation() {
        let gateway = gateway();
        let approvals = MemoryApprovals::default();
        let clock = FixedClock(100);
        let token = FixedToken;
        let preview = prepare(&gateway, &approvals, &clock, &token);

        service(&gateway, &approvals, &clock, &token)
            .confirm(&preview.token, "Changed response")
            .unwrap_err();
        assert!(gateway.dispatched.borrow().is_empty());

        gateway.conversation.borrow_mut().stable_id = "stable-2".to_owned();
        service(&gateway, &approvals, &clock, &token)
            .confirm(&preview.token, "Confirmed response")
            .unwrap_err();
        assert!(gateway.dispatched.borrow().is_empty());
    }

    #[test]
    fn confirmed_approval_is_single_use() {
        let gateway = gateway();
        let approvals = MemoryApprovals::default();
        let clock = FixedClock(100);
        let token = FixedToken;
        let preview = prepare(&gateway, &approvals, &clock, &token);

        service(&gateway, &approvals, &clock, &token)
            .confirm(&preview.token, "Confirmed response")
            .unwrap();
        assert_eq!(
            gateway.dispatched.borrow().as_slice(),
            ["Confirmed response"]
        );
        service(&gateway, &approvals, &clock, &token)
            .confirm(&preview.token, "Confirmed response")
            .unwrap_err();
    }

    #[test]
    fn expired_and_cancelled_approvals_cannot_dispatch() {
        let gateway = gateway();
        let approvals = MemoryApprovals::default();
        let created = FixedClock(100);
        let token = FixedToken;
        let preview = prepare(&gateway, &approvals, &created, &token);
        let expired = FixedClock(2_000);

        service(&gateway, &approvals, &expired, &token)
            .confirm(&preview.token, "Confirmed response")
            .unwrap_err();
        assert!(approvals.0.borrow().is_empty());
        assert!(gateway.dispatched.borrow().is_empty());

        let second = prepare(&gateway, &approvals, &created, &token);
        assert!(
            service(&gateway, &approvals, &created, &token)
                .cancel(&second.token)
                .unwrap()
        );
        assert!(
            service(&gateway, &approvals, &created, &token)
                .confirm(&second.token, "Confirmed response")
                .is_err()
        );
    }
}
