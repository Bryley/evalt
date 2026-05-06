use crate::{
    output::{json::JsonWriter, terminal::TerminalWriter},
    types::{test::Test, yaml_spec::TestFile},
};
use std::{path::PathBuf, sync::OnceLock};

mod terminal;
mod json;

static REPORT_WRITER: OnceLock<Box<dyn ReportWriter>> = OnceLock::new();

pub trait ReportWriter: Send + Sync {
    /// Output when files are checked, `error_only` is used for if we just want to report errors (if
    /// parse errors occur during run stage)
    ///
    /// # Returns
    /// Error code for application
    fn check_files(
        &self,
        error_only: bool,
        results: &[(PathBuf, miette::Result<TestFile>)],
    ) -> anyhow::Result<i32>;

    /// When the test information has updated and should be displayed
    fn update_test(&self, test: &Test) -> anyhow::Result<()>;

    /// Output final state of tests
    ///
    /// # Returns
    /// Error code for application
    fn finish_tests(&self, tests: &[Test]) -> anyhow::Result<i32>;
}

pub fn init_report_writer(json: bool) -> &'static dyn ReportWriter {
    REPORT_WRITER
        .get_or_init(|| {
            if json {
                Box::new(JsonWriter)
            } else {
                Box::new(TerminalWriter::new())
            }
        })
        .as_ref()
}

pub fn report_writer() -> &'static dyn ReportWriter {
    let Some(term_output) = REPORT_WRITER.get() else {
        return init_report_writer(false);
    };

    term_output.as_ref()
}
