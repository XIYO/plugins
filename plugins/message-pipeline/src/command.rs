use std::{
    ffi::OsString,
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};

use anyhow::{Result, anyhow, bail};
use tracing::{error, info};

#[derive(Debug)]
pub struct ProcessOutput {
    pub stdout: Vec<u8>,
}

pub trait CommandRunner: Send + Sync {
    fn run(
        &self,
        boundary: &'static str,
        program: &Path,
        args: &[OsString],
    ) -> Result<ProcessOutput>;
}

#[derive(Debug, Default)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(
        &self,
        boundary: &'static str,
        program: &Path,
        args: &[OsString],
    ) -> Result<ProcessOutput> {
        info!(
            boundary,
            argument_count = args.len(),
            "[extract:command:start] starting read-only source command"
        );
        let started = Instant::now();
        let output = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        let output = match output {
            Ok(output) => output,
            Err(error_value) => {
                error!(
                    boundary,
                    error_kind = ?error_value.kind(),
                    "[extract:command:failure] source command could not start"
                );
                return Err(anyhow!("source reader could not start: {error_value}"));
            }
        };

        if !output.status.success() {
            error!(
                boundary,
                exit_code = output.status.code(),
                stderr_bytes = output.stderr.len(),
                duration_ms = started.elapsed().as_millis(),
                "[extract:command:failure] source command failed"
            );
            bail!(
                "source reader failed at {boundary} with exit code {:?}; run the reader directly for diagnostics",
                output.status.code()
            );
        }

        info!(
            boundary,
            stdout_bytes = output.stdout.len(),
            stderr_bytes = output.stderr.len(),
            duration_ms = started.elapsed().as_millis(),
            "[extract:command:success] source command completed"
        );
        Ok(ProcessOutput {
            stdout: output.stdout,
        })
    }
}
