use std::{
    ffi::OsString,
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};

use anyhow::{Context, Result, bail};
use tracing::{error, info};

#[derive(Debug)]
pub struct ProcessOutput {
    pub stdout: Vec<u8>,
}

pub trait ProcessExecutor {
    fn run(
        &self,
        boundary: &'static str,
        program: &Path,
        arguments: &[OsString],
    ) -> Result<ProcessOutput>;
}

#[derive(Debug, Default)]
pub struct SystemProcessExecutor;

impl ProcessExecutor for SystemProcessExecutor {
    fn run(
        &self,
        boundary: &'static str,
        program: &Path,
        arguments: &[OsString],
    ) -> Result<ProcessOutput> {
        info!(
            boundary,
            argument_count = arguments.len(),
            "[adapter:process:start] starting external adapter"
        );
        let started = Instant::now();
        let output = Command::new(program)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| format!("unable to start adapter at {boundary}"))?;
        if !output.status.success() {
            error!(
                boundary,
                exit_code = output.status.code(),
                stderr_bytes = output.stderr.len(),
                duration_ms = started.elapsed().as_millis(),
                "[adapter:process:failure] external adapter failed"
            );
            bail!(
                "adapter failed at {boundary} with exit code {:?}",
                output.status.code()
            )
        }
        info!(
            boundary,
            stdout_bytes = output.stdout.len(),
            stderr_bytes = output.stderr.len(),
            duration_ms = started.elapsed().as_millis(),
            "[adapter:process:success] external adapter completed"
        );
        Ok(ProcessOutput {
            stdout: output.stdout,
        })
    }
}
