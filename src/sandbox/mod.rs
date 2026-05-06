use std::path::Path;

use anyhow::Result;
use miette::Error;

use crate::types::CommandOutput;

pub mod cage;

pub enum SandboxControl {
    Continue,
    Cancel { reason: Error, timeout: bool },
}

pub const SANDBOX_ENVIRONMENTS: &str = "/var/tmp/evalt-sandboxes";

pub trait Sandbox: Send + Sync {
    /// The unique ID of the sandbox
    fn id(&self) -> &'static str;

    /// Is the sandbox already prepared
    fn is_prepared(&self) -> bool;

    /// Called to prepare the sandbox environment
    fn prepare(&self) -> impl Future<Output = Result<()>>;

    /// Run a command inside the sandbox environment
    /// `workspace` is expected to be a prepared folder that will be edited.
    ///
    /// # Returns
    /// - None - if the command succeeded
    /// - Some(Report) - if the command timed out or exceeded max budgets
    /// - Err(Report) - if the command failed or cancelled for any reason outside timing out
    fn run<F>(
        &self,
        workspace: &Path,
        extra_allowed_paths: &[&Path],
        cmd: &[&str],
        on_update: F,
    ) -> impl Future<Output = miette::Result<Option<miette::Error>>>
    where
        F: FnMut(CommandOutput) -> SandboxControl + Send;

    /// Cleanup the session (removing workspace, so on)
    fn cleanup(&self) -> impl Future<Output = Result<()>>;
}
