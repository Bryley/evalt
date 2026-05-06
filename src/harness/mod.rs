use std::path::Path;

use crate::types::{CommandOutput, test::TestRun};
use anyhow::Result;

pub mod pi;

pub trait HarnessAdaptor: Sync {
    fn parse_result(&self, test_run: &mut TestRun, event: CommandOutput) -> Result<()>;
    fn get_run_command(
        &self,
        system_prompt: Option<&str>,
        workspace: &Path,
        extra_args: &[&str],
        prompt: &str,
        is_review: bool,
    ) -> Result<Vec<String>>;
}
