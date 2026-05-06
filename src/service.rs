use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{sandbox::Sandbox as _, utils::AnyhowExt as _};
use futures::{StreamExt, stream};
use ignore::WalkBuilder;
use miette::Context;

use crate::{
    engine::Engine,
    types::{config::stack::Stack, test::Test, yaml_spec::TestFile},
};

pub struct Service {
    config: Stack,
}

/// Result of a run.
///
/// Ok => a list of successfully ran tests containing run info inside
/// Err => a list of tests found but failed validation, their path and the error accociated with them.
pub type RunResult = Result<Vec<Test>, Vec<(PathBuf, miette::Error)>>;

impl Service {
    pub fn new(profile: Option<String>, reviewer_profile: Option<String>) -> miette::Result<Self> {
        Ok(Self {
            config: Stack::new(profile, reviewer_profile)?,
        })
    }

    /// Finds and runs tests
    ///
    /// # Returns
    /// List of tests or None if failed to parse some tests and should res
    pub async fn run(&self, path: &Path, selectors: &[String]) -> miette::Result<RunResult> {
        let mut test_files = Vec::new();
        let mut failures = Vec::new();
        for (path, result) in self.fetch_tests_files(path, selectors) {
            match result {
                Ok(test_file) => test_files.push(test_file),
                Err(err) => {
                    failures.push((path, err));
                }
            }
        }
        if !failures.is_empty() {
            return Ok(Err(failures));
        }

        let sandbox = self.config.sandbox();
        sandbox.prepare().await.into_miette()?;

        let mut tests = Vec::new();

        for test_file in test_files {
            let test_file_stack = self.config.with_opt_config(test_file.config);
            for test_case in test_file.tests {
                let test: Test = test_case.into();
                tests.push((test, test_file_stack.clone()));
            }
        }

        let engine = &self.engine();
        let tests = stream::iter(tests)
            .map(|(mut test, stack)| async move {
                test.to_running();
                let mut test = engine.run_test(&stack, test).await;
                if test.is_finished() {
                    return test;
                }
                test.to_reviewing();
                let finished_state = engine.run_review(&stack, &mut test).await;
                test.to_finish(finished_state);
                test
            })
            .buffer_unordered(10)
            .collect::<Vec<_>>()
            .await;

        Ok(Ok(tests))
    }

    pub fn check(&self, path: &Path) -> Vec<(PathBuf, miette::Result<TestFile>)> {
        self.fetch_tests_files(path, &[])
    }

    fn engine(&self) -> Engine {
        Engine
    }

    fn find_test_files(&self, root: &Path) -> Vec<PathBuf> {
        // TODO make a lot of these things global options or something and global custom ignore file
        WalkBuilder::new(root)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
            .map(|entry| entry.into_path())
            .filter(|path| {
                let name = path
                    .file_name()
                    .and_then(|os_str| os_str.to_str())
                    .unwrap_or_default();
                name.ends_with(".eval.yaml") || name.ends_with(".eval.yml")
            })
            .collect()
    }

    fn parse_test_file(
        &self,
        path: &Path,
        selectors: &[String],
    ) -> std::result::Result<TestFile, miette::Report> {
        let Ok(contents) = fs::read_to_string(path) else {
            panic!(
                "{path:?} could not be read, possibly because it doesn't exist of because of perms"
            );
        };
        let mut test_file = match serde_saphyr::from_str::<TestFile>(&contents) {
            Ok(test_file) => test_file,
            Err(err) => {
                let report = serde_saphyr::miette::to_miette_report_with_formatter(
                    &err,
                    &contents,
                    &path.display().to_string(),
                    &serde_saphyr::UserMessageFormatter,
                );
                return Err(report);
            }
        };

        test_file.set_path(path);
        test_file.apply_selectors(selectors);
        test_file.validate()?;
        Ok(test_file)
    }

    fn fetch_tests_files(
        &self,
        root: &Path,
        selectors: &[String],
    ) -> Vec<(PathBuf, miette::Result<TestFile>)> {
        self.find_test_files(root)
            .into_iter()
            .map(|path| {
                let testfile = self
                    .parse_test_file(&path, selectors)
                    .with_context(|| format!("file path: {}", path.display()));
                (path, testfile)
            })
            .collect::<Vec<_>>()
    }
}
