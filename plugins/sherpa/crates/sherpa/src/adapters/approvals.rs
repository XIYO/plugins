use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use directories::BaseDirs;

use crate::context::reply::{ApprovalRecord, ApprovalRepository, validate_token};

#[derive(Debug)]
pub struct FileApprovalRepository {
    directory: PathBuf,
}

impl FileApprovalRepository {
    pub fn from_environment() -> Result<Self> {
        let directory = match std::env::var_os("SHERPA_CONTEXT_REPLY_STATE") {
            Some(path) => PathBuf::from(path),
            None => {
                let state_root = match std::env::var_os("XDG_STATE_HOME") {
                    Some(path) => PathBuf::from(path),
                    None => BaseDirs::new()
                        .context("unable to resolve the local state directory")?
                        .home_dir()
                        .join(".local/state"),
                };
                state_root.join("sherpa/context/replies")
            }
        };
        Self::new(directory)
    }

    pub fn new(directory: PathBuf) -> Result<Self> {
        ensure_private_directory(&directory)?;
        Ok(Self { directory })
    }

    fn path(&self, token: &str) -> Result<PathBuf> {
        validate_token(token)?;
        Ok(self.directory.join(format!("{token}.json")))
    }
}

impl ApprovalRepository for FileApprovalRepository {
    fn remove_expired(&self, now: i64) -> Result<()> {
        for entry in fs::read_dir(&self.directory).context("unable to inspect approval state")? {
            let entry = entry.context("unable to inspect an approval entry")?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let metadata = fs::symlink_metadata(&path)
                .with_context(|| format!("unable to inspect {}", path.display()))?;
            if !metadata.file_type().is_file()
                || metadata.uid() != current_uid()
                || metadata.permissions().mode() & 0o077 != 0
            {
                continue;
            }
            let record = fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<ApprovalRecord>(&bytes).ok());
            if record.is_some_and(|record| record.expires_at <= now) {
                fs::remove_file(&path)
                    .with_context(|| format!("unable to remove expired {}", path.display()))?;
            }
        }
        Ok(())
    }

    fn save(&self, token: &str, record: &ApprovalRecord) -> Result<()> {
        let path = self.path(token)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .context("unable to create an approval record")?;
        let result = (|| -> Result<()> {
            serde_json::to_writer(&mut file, record).context("unable to encode approval record")?;
            file.write_all(b"\n")
                .context("unable to finish approval record")?;
            file.sync_all().context("unable to persist approval record")
        })();
        if result.is_err() {
            let _ = fs::remove_file(path);
        }
        result
    }

    fn load(&self, token: &str) -> Result<ApprovalRecord> {
        let path = self.path(token)?;
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            anyhow::anyhow!("approval token is missing or already used: {error}")
        })?;
        if !metadata.file_type().is_file() || metadata.uid() != current_uid() {
            bail!("approval record must be a private regular file owned by the current user")
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            bail!("approval record must have mode 0600")
        }
        let bytes = fs::read(path).context("unable to read approval record")?;
        serde_json::from_slice(&bytes).context("unable to decode approval record")
    }

    fn delete(&self, token: &str) -> Result<bool> {
        let path = self.path(token)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error).context("unable to remove approval record"),
        }
    }
}

fn ensure_private_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        let mut builder = DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder
            .create(path)
            .context("unable to create approval state directory")?;
    }
    let metadata =
        fs::symlink_metadata(path).context("unable to inspect approval state directory")?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        bail!("approval state path must be a real directory")
    }
    if metadata.uid() != current_uid() {
        bail!("approval state directory must be owned by the current user")
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        bail!("approval state directory must have mode 0700")
    }
    Ok(())
}

fn current_uid() -> u32 {
    rustix::process::getuid().as_raw()
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn repository_uses_private_permissions() {
        let root = tempdir().unwrap();
        let directory = root.path().join("private");
        let repository = FileApprovalRepository::new(directory.clone()).unwrap();
        let record = ApprovalRecord {
            version: 1,
            channel: "kakaotalk".to_owned(),
            conversation_name: "Example".to_owned(),
            conversation_id_sha256: "a".repeat(64),
            message_sha256: "b".repeat(64),
            message_length: 1,
            created_at: 1,
            expires_at: 2,
        };
        let token = "0123456789abcdef0123456789abcdef";

        repository.save(token, &record).unwrap();

        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(directory.join(format!("{token}.json")))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
