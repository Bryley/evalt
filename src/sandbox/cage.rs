use miette::{Context, IntoDiagnostic};
use std::{fs::canonicalize, iter, path::Path, process::Stdio};

use tokio::{
    io::{AsyncBufReadExt as _, BufReader},
    process::Command,
};

use crate::{
    sandbox::{Sandbox, SandboxControl},
    types::{CommandOutput, OutputStream}, utils::clip,
};

pub struct CageSandbox;

impl Sandbox for CageSandbox {
    fn id(&self) -> &'static str {
        "cage"
    }

    fn is_prepared(&self) -> bool {
        true
    }

    async fn prepare(&self) -> anyhow::Result<()> {
        which::which("cage").map_err(|_| {
            anyhow::anyhow!("`cage` binary must be installed and available on the system")
        })?;

        Ok(())
    }

    async fn run<F>(
        &self,
        workspace: &Path,
        extra_allowed_paths: &[&Path],
        cmd: &[&str],
        on_update: F,
    ) -> miette::Result<Option<miette::Error>>
    where
        F: FnMut(crate::types::CommandOutput) -> super::SandboxControl + Send,
    {
        let mut command = Command::new("cage");

        let full_allowed = iter::once(&workspace)
            .chain(extra_allowed_paths.iter())
            .collect::<Vec<_>>();

        for allowed_path in full_allowed.iter() {
            let allowed_path = canonicalize(allowed_path)
                .into_diagnostic()
                .with_context(|| {
                    format!("couldn't canonicalize path '{}'", allowed_path.display())
                })?;
            command.arg("-allow").arg(allowed_path);
        }
        command.arg("--");
        command.current_dir(workspace);

        stream_command_to_callback(&mut command, cmd, on_update)
            .await
            .with_context(move || {
                let paths = full_allowed
                    .iter()
                    .map(|x| x.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("failed streaming sandbox command allowing paths, allowed paths: {paths}",)
            })
    }

    async fn cleanup(&self) -> anyhow::Result<()> {
        // Nothing to cleanup
        Ok(())
    }
}

/// Streams the command callback style
///
/// # Returns
/// - None - if the stream succeeded
/// - Some(Report) - if the stream timed out or exceeded max budgets
/// - Err(Report) - if the command failed or canceled for any reason outside timing out
async fn stream_command_to_callback<F>(
    command: &mut Command,
    cmd: &[&str],
    mut on_update: F,
) -> miette::Result<Option<miette::Error>>
where
    F: FnMut(crate::types::CommandOutput) -> super::SandboxControl + Send,
{
    let mut child = command
        .args(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .into_diagnostic()
        .with_context(|| {
            let cmd = cmd
                .iter()
                .map(|part| {
                    if part.contains(" ") {
                        format!("\"{part}\"")
                    } else {
                        part.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            let cmd = clip(&cmd, 1000);

            format!("failed to sandbox command: {cmd}")
        })?;

    let mut stderr_buffer = String::new();

    let stdout = child
        .stdout
        .take()
        .wrap_err("failed to open child stdout")?;
    let stderr = child
        .stderr
        .take()
        .wrap_err("failed to open child stderr")?;

    let mut stdout = BufReader::new(stdout).lines();
    let mut stderr = BufReader::new(stderr).lines();

    let mut stdout_done = false;
    let mut stderr_done = false;
    let mut cancel_reason = None;

    while (!stdout_done || !stderr_done) && cancel_reason.is_none() {
        tokio::select! {
            line = stdout.next_line(), if !stdout_done => {
                if let Some(line) = line.into_diagnostic().context("io error reading line")? {
                    let result = on_update(CommandOutput {
                        stream: OutputStream::Stdout,
                        line
                    });
                    if let SandboxControl::Cancel{ reason, timeout } = result {
                        cancel_reason = Some((reason, timeout));
                    }

                } else {
                    stdout_done = true;
                }
            },
            line = stderr.next_line(), if !stderr_done => {
                if let Some(line) = line.into_diagnostic().context("io error reading line")? {
                    stderr_buffer.push_str(&line);
                    stderr_buffer.push('\n');
                    let result = on_update(CommandOutput {
                        stream: OutputStream::Stderr,
                        line
                    });
                    if let SandboxControl::Cancel{ reason, timeout } = result {
                        cancel_reason = Some((reason, timeout));
                    }
                } else {
                    stderr_done = true;
                }
            },
        }
    }

    if let Some((reason, timeout)) = cancel_reason {
        child
            .kill()
            .await
            .into_diagnostic()
            .context("failed to kill child process")?;

        if timeout {
            return Ok(Some(reason.context("timeout")));
        }

        return Err(reason);
    }

    let status = child
        .wait()
        .await
        .into_diagnostic()
        .context("failed to wait for child process to finish")?;

    if !status.success() {
        miette::bail!("{stderr_buffer}");
    }

    Ok(None)
}
