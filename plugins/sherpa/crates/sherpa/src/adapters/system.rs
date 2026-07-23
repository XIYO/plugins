use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::context::reply::{Clock, TokenGenerator};

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or(0)
    }
}

#[derive(Debug, Default)]
pub struct SecureTokenGenerator;

impl TokenGenerator for SecureTokenGenerator {
    fn generate(&self) -> Result<String> {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).context("unable to generate a secure approval token")?;
        Ok(hex::encode(bytes))
    }
}
