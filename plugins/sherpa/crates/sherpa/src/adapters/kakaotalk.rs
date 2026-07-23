use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use tracing::{error, info, warn};

use crate::{
    context::reply::{Conversation, ConversationGateway},
    process::{ProcessExecutor, SystemProcessExecutor},
};

const CONVERSATION_SCAN_LIMIT: usize = 10_000;

#[derive(Debug)]
pub struct KakaoTalkGateway {
    binary: PathBuf,
    executor: SystemProcessExecutor,
}

#[derive(Debug, Deserialize)]
struct KakaoTalkConversation {
    id: Value,
    display_name: String,
}

impl KakaoTalkGateway {
    pub fn from_environment() -> Result<Self> {
        let binary = resolve_binary(
            std::env::var_os("KAKAOCLI_BIN").map(PathBuf::from),
            "kakaocli",
        )?;
        Ok(Self {
            binary,
            executor: SystemProcessExecutor,
        })
    }

    fn conversations(&self) -> Result<Vec<KakaoTalkConversation>> {
        info!("[context:reply:resolve:start] resolving the exact KakaoTalk conversation");
        let output = self.executor.run(
            "context.kakaotalk.conversations",
            &self.binary,
            &[
                OsString::from("chats"),
                OsString::from("--limit"),
                OsString::from(CONVERSATION_SCAN_LIMIT.to_string()),
                OsString::from("--json"),
            ],
        )?;
        serde_json::from_slice(&output.stdout)
            .context("KakaoTalk conversation discovery returned invalid JSON")
    }
}

impl ConversationGateway for KakaoTalkGateway {
    fn resolve_exact(&self, display_name: &str) -> Result<Conversation> {
        if display_name.trim().is_empty() {
            bail!("an exact conversation name is required")
        }
        let conversations = self.conversations()?;
        let requested = display_name.to_lowercase();
        let exact = conversations
            .iter()
            .filter(|conversation| conversation.display_name == display_name)
            .collect::<Vec<_>>();
        let partial = conversations
            .iter()
            .filter(|conversation| {
                conversation
                    .display_name
                    .to_lowercase()
                    .contains(&requested)
            })
            .collect::<Vec<_>>();
        if exact.len() != 1 || partial.len() != 1 {
            warn!(
                exact_matches = exact.len(),
                partial_matches = partial.len(),
                "[context:reply:resolve:rejected] refusing a missing or ambiguous conversation"
            );
            bail!("conversation is missing or ambiguous; use one unique exact display name")
        }
        let conversation = exact[0];
        let stable_id = match &conversation.id {
            Value::String(value) => value.clone(),
            Value::Number(value) => value.to_string(),
            _ => bail!("resolved conversation has an invalid stable identifier"),
        };
        if stable_id.is_empty() {
            bail!("resolved conversation is missing its stable identifier")
        }
        info!("[context:reply:resolve:success] resolved one exact KakaoTalk conversation");
        Ok(Conversation {
            stable_id,
            display_name: conversation.display_name.clone(),
        })
    }

    fn dispatch_text(&self, conversation: &Conversation, message: &str) -> Result<()> {
        info!("[context:reply:dispatch:start] dispatching confirmed KakaoTalk text");
        let result = self.executor.run(
            "context.kakaotalk.dispatch",
            &self.binary,
            &[
                OsString::from("send"),
                OsString::from(&conversation.display_name),
                OsString::from(message),
            ],
        );
        match result {
            Ok(_) => {
                info!("[context:reply:dispatch:success] confirmed KakaoTalk text was dispatched");
                Ok(())
            }
            Err(error_value) => {
                error!(
                    error = ?error_value,
                    "[context:reply:dispatch:failure] KakaoTalk UI dispatch failed"
                );
                Err(error_value).context("KakaoTalk UI dispatch failed")
            }
        }
    }
}

fn resolve_binary(configured: Option<PathBuf>, command: &str) -> Result<PathBuf> {
    if let Some(path) = configured {
        if executable(&path) {
            return Ok(path);
        }
        bail!("configured {command} is not executable")
    }
    if let Some(path) = managed_binary(command)
        && executable(&path)
    {
        return Ok(path);
    }
    which::which(command).with_context(|| format!("{command} is required"))
}

fn managed_binary(command: &str) -> Option<PathBuf> {
    let root = std::env::var_os("SHERPA_INSTALL_ROOT")
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".local")))?;
    Some(root.join("bin").join(command))
}

fn executable(path: &Path) -> bool {
    path.is_file()
        && std::fs::metadata(path)
            .map(|metadata| {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            })
            .unwrap_or(false)
}
