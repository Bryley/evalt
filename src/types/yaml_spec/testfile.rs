use std::collections::HashSet;
use std::fmt::Display;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Deserialize;

use super::config::Config;

use super::assertions::Assertion;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TestFile {
    #[serde(skip)]
    #[schemars(skip)]
    path: PathBuf,
    /// The version of the test file
    #[allow(unused)]
    version: u32,
    /// Config options for the test
    pub config: Option<Config>,
    /// List of tests
    pub tests: Vec<TestCase>,
}

impl TestFile {
    pub fn validate(&self) -> miette::Result<()> {
        if self
            .config
            .as_ref()
            .and_then(|x| x.run.as_ref())
            .and_then(|x| x.concurrent_tests)
            .is_some()
        {
            miette::bail!(
                help = "you must set it on the global config file if you want to change it",
                "cannot set `concurrent-tests` option on the test file level"
            )
        }

        let mut ids = HashSet::new();
        for test in &self.tests {
            if test
                .config
                .as_ref()
                .and_then(|x| x.run.as_ref())
                .and_then(|x| x.concurrent_tests)
                .is_some()
            {
                miette::bail!(
                    help = "you must set it on the global config file if you want to change it",
                    "cannot set `concurrent-tests` option on the individual test level"
                )
            }

            for assertion in &test.assertions {
                assertion.validate()?;
            }

            let id = test.id();

            if ids.contains(&id) {
                miette::bail!("contains duplicate test ids: {id}");
            }

            ids.insert(id);
        }
        Ok(())
    }

    pub fn set_path(&mut self, path: &Path) {
        self.path = path.to_path_buf();
        for test in &mut self.tests {
            test.path = path.to_path_buf();
        }
    }

    pub fn get_path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn apply_selectors(&mut self, selectors: &[String]) {
        if selectors.is_empty() {
            return;
        }
        self.tests.retain(|test| {
            selectors
                .iter()
                .any(|selector| test.name.contains(selector))
        });
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TestCase {
    #[serde(skip)]
    #[schemars(skip)]
    pub path: PathBuf,

    /// The name of the test
    pub name: String,
    /// The test's description
    pub desc: Option<String>,
    /// Additional config options for this test
    pub config: Option<Config>,
    /// The workspace fixtures configuration
    pub workspace: Option<Workspace>,
    /// The input prompt into the test
    pub input: Input,
    /// List of assertions for test
    pub assertions: Vec<Assertion>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TestId {
    pub path: PathBuf,
    pub name: String,
}

impl Display for TestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = self
            .path
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("no-name");
        let path = path
            .trim_end_matches(".eval.yaml")
            .trim_end_matches(".eval.yml");
        write!(f, "{path}-{}", self.name)
    }
}

impl TestCase {
    pub fn id(&self) -> TestId {
        TestId {
            path: self.path.clone(),
            name: self.name.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Workspace {
    pub copy: Vec<WorkspaceItem>,
    // TODO add ignore options
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct WorkspaceItem {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Input {
    Prompt(String),
    // TODO allow for more structured
}

#[cfg(test)]
mod tests {
    use super::{
        super::assertions::{Assertion, ComparisonAssertion},
        super::operations::StringOp,
        TestFile,
    };

    fn parse_test_file(yaml: &str) -> TestFile {
        match serde_saphyr::from_str(yaml) {
            Ok(x) => x,
            Err(error) => {
                let report = serde_saphyr::miette::to_miette_report_with_formatter(
                    &error,
                    yaml,
                    "",
                    &serde_saphyr::UserMessageFormatter,
                );
                panic!("{report:?}");
            }
        }
    }

    #[test]
    fn parses_basic_comparison_assertion() {
        let file = parse_test_file(
            r#"
version: 1
tests:
  - name: basic
    input:
      prompt: "hello"
    assertions:
      - left: output
        op: contains
        right: "hello"
"#,
        );

        assert_eq!(file.tests.len(), 1);
        assert_eq!(file.tests[0].assertions.len(), 1);

        match &file.tests[0].assertions[0] {
            Assertion::Comparison {
                comparison: ComparisonAssertion::OutputAssistant { op },
            } => {
                assert!(
                    matches!(op, StringOp::Contains{ right: x, case_sensitive: false, } if x == "hello" )
                );
            }
            _ => panic!("unexpected assertion variant"),
        }
    }

    #[test]
    fn parses_nested_assertions() {
        let file = parse_test_file(
            r#"
version: 1
tests:
  - name: nested
    input:
      prompt: "hello"
    assertions:
      - all:
          - left: turns
            op: <=
            right: 3
          - all:
              - left: output
                op: contains
                right: "hello"
"#,
        );

        match &file.tests[0].assertions[0] {
            Assertion::All { all } => {
                assert_eq!(all.len(), 2);
                assert!(matches!(
                    all[0],
                    Assertion::Comparison {
                        comparison: ComparisonAssertion::Turns { .. }
                    }
                ));
                assert!(matches!(all[1], Assertion::All { .. }));
            }
            _ => panic!("unexpected assertion variant"),
        }
    }

    #[test]
    fn parses_mixed_review_and_comparison_assertions() {
        let file = parse_test_file(
            r#"
version: 1
tests:
  - name: mixed
    input:
      prompt: "hello"
    assertions:
      - left: output
        op: contains
        right: "hello"
      - review:
          prompt: "Did the assistant follow instructions?"
          pass_threshold: 0.8
"#,
        );

        assert_eq!(file.tests[0].assertions.len(), 2);
        assert!(matches!(
            file.tests[0].assertions[0],
            Assertion::Comparison {
                comparison: ComparisonAssertion::OutputAssistant { .. }
            }
        ));
        assert!(matches!(
            file.tests[0].assertions[1],
            Assertion::Review { .. }
        ));
    }

    #[test]
    fn validate_rejects_file_level_concurrent_tests() {
        let file = parse_test_file(
            r#"
version: 1
config:
  run:
    concurrent-tests: 15
tests:
  - name: mixed
    input:
      prompt: "hello"
    assertions:
      - left: output
        op: contains
        right: "hello"
"#,
        );

        let error = file.validate().expect_err("validation should fail");
        assert_eq!(
            error.to_string(),
            "cannot set `concurrent-tests` option on the test file level"
        );
    }

    #[test]
    fn validate_rejects_test_level_concurrent_tests() {
        let file = parse_test_file(
            r#"
version: 1
tests:
  - name: mixed
    config:
      run:
        concurrent-tests: 15
    input:
      prompt: "hello"
    assertions:
      - left: output
        op: contains
        right: "hello"
"#,
        );

        let error = file.validate().expect_err("validation should fail");
        assert_eq!(
            error.to_string(),
            "cannot set `concurrent-tests` option on the individual test level"
        );
    }

    #[test]
    fn validate_accepts_non_global_only_run_config() {
        let file = parse_test_file(
            r#"
version: 1
config:
  run:
    timeout-ms: 1000
tests:
  - name: mixed
    config:
      run:
        max-turns: 3
    input:
      prompt: "hello"
    assertions:
      - left: output
        op: contains
        right: "hello"
"#,
        );

        file.validate().expect("validation should pass");
    }

    #[test]
    fn validate_duplicate_test_ids() {
        let file = parse_test_file(
            r#"
version: 1
tests:
  - name: mixed
    input:
      prompt: "hello"
    assertions:
      - left: output
        op: contains
        right: "hello"
  - name: mixed
    input:
      prompt: "hello"
    assertions:
      - left: output
        op: contains
        right: "hello"
"#,
        );

        let error = file.validate().expect_err("validation should fail");
        assert_eq!(
            error.to_string(),
            "contains duplicate test ids: no-name-mixed"
        );
    }
}
