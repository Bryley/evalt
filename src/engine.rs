use crate::{
    harness::HarnessAdaptor,
    output::report_writer,
    sandbox::{SANDBOX_ENVIRONMENTS, Sandbox, SandboxControl},
    types::{
        config::stack::Stack,
        test::{FinishedState, Test, TestRun, TestState},
        testresult::AssertionResult,
        yaml_spec::{Input, TestCase, assertions::ReviewAssertion, testfile::TestId},
    },
    utils::AnyhowExt,
};
use anyhow::{Context, Result, bail};
use ignore::WalkBuilder;
use miette::Context as _;
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct Engine;

impl Engine {
    fn setup_test_workspace(&self, id: &TestId) -> anyhow::Result<PathBuf> {
        let name = format!("workspace-{}", id);
        let workspace_dir = Path::new(SANDBOX_ENVIRONMENTS).join(name);
        fs::create_dir_all(&workspace_dir).with_context(|| {
            format!(
                "failed to create temp workspace at {}",
                workspace_dir.display()
            )
        })?;
        Ok(workspace_dir)
    }

    pub fn prepare_workspace_from_testcase(&self, test_case: &TestCase) -> Result<PathBuf> {
        let workspace_dir = self.setup_test_workspace(&test_case.id())?;

        let Some(workspace) = &test_case.workspace else {
            return Ok(workspace_dir);
        };

        let test_case_dir = test_case.path.parent().unwrap_or(Path::new("."));

        for item in &workspace.copy {
            let from = Path::new(&item.from);
            let from = if from.is_absolute() {
                from.to_path_buf()
            } else {
                test_case_dir.join(from)
            };
            if !from.exists() {
                bail!("workspace.copy source does not exist: {}", from.display());
            }

            let to = workspace_dir.join(&item.to);

            if from.is_file() {
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent).with_context(|| {
                        format!("failed to create parent dir for {}", to.display())
                    })?;
                }
                fs::copy(&from, &to).with_context(|| {
                    format!("failed to copy file {} -> {}", from.display(), to.display())
                })?;
                continue;
            }

            for entry in WalkBuilder::new(&from)
                .hidden(false)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .build()
            {
                let entry = entry?;
                let src_path = entry.path();

                let rel = src_path.strip_prefix(&from).with_context(|| {
                    format!(
                        "failed to strip prefix {} from {}",
                        from.display(),
                        src_path.display()
                    )
                })?;
                let dest_path = to.join(rel);

                if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                    fs::create_dir_all(&dest_path).with_context(|| {
                        format!("failed to create directory {}", dest_path.display())
                    })?;
                } else if entry.file_type().is_some_and(|ft| ft.is_file()) {
                    if let Some(parent) = dest_path.parent() {
                        fs::create_dir_all(parent).with_context(|| {
                            format!("failed to create parent dir for {}", dest_path.display())
                        })?;
                    }
                    fs::copy(src_path, &dest_path).with_context(|| {
                        format!(
                            "failed to copy file {} -> {}",
                            src_path.display(),
                            dest_path.display()
                        )
                    })?;
                }
            }
        }

        Ok(workspace_dir)
    }

    pub async fn run_test(&self, test_file_stack: &Stack, mut test: Test) -> Test {
        let stack = test_file_stack.with_opt_config(test.test_case.config.clone());
        let config = match stack.resolve().context("failed to resolve config") {
            Ok(x) => x,
            Err(err) => {
                return test.error(err);
            }
        };
        let workspace = match self
            .prepare_workspace_from_testcase(&test.test_case)
            .into_miette()
            .context("failed to prepare workspace for testcase")
        {
            Ok(x) => x,
            Err(err) => {
                return test.error(err);
            }
        };
        test.workspace = Some(workspace.clone());
        test.profile = Some(config.profile.clone());

        match &test.test_case.input.clone() {
            Input::Prompt(prompt) => {
                let sandbox = config.sandbox();
                let harness_adaptor = config.harness_adapter();

                let extra_args = config
                    .profile
                    .extra_args
                    .iter()
                    .map(|x| x.as_str())
                    .collect::<Vec<_>>();
                let cmd = match harness_adaptor
                    .get_run_command(None, workspace.as_path(), &extra_args, prompt, false)
                    .into_miette()
                    .context("failed getting run command")
                {
                    Ok(x) => x,
                    Err(err) => {
                        return test.error(err);
                    }
                };

                let extra_allowed = match config.profile.harness.extra_allowed_paths().into_miette()
                {
                    Ok(x) => x,
                    Err(err) => {
                        return test.error(err);
                    }
                };
                let result = sandbox
                    .run(
                        &workspace,
                        &extra_allowed
                            .iter()
                            .map(|x| x.as_path())
                            .collect::<Vec<_>>(),
                        &cmd.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
                        |command_output| {
                            if let Err(err) = harness_adaptor
                                .parse_result(&mut test.run, command_output)
                                .into_miette()
                            {
                                return SandboxControl::Cancel {
                                    reason: err,
                                    timeout: false,
                                };
                            };

                            if let Err(err) = test.run.timeout_reached(&config.run) {
                                return SandboxControl::Cancel {
                                    reason: err,
                                    timeout: true,
                                };
                            };

                            if let Err(err) = report_writer().update_test(&test).into_miette() {
                                return SandboxControl::Cancel {
                                    reason: err,
                                    timeout: false,
                                };
                            };
                            SandboxControl::Continue
                        },
                    )
                    .await;

                match result {
                    Ok(Some(timeout_err)) => {
                        return test.timeout(timeout_err, false);
                    }
                    Err(err) => {
                        return test.error(err);
                    }
                    _ => {}
                }
            }
        };

        test
    }

    pub async fn run_review(&self, test_file_stack: &Stack, test: &mut Test) -> FinishedState {
        let TestState::Reviewing {
            start_time,
            run_duration,
        } = test.state
        else {
            unreachable!("function can only be called on a reviewing test");
        };

        let assertions = test
            .test_case
            .assertions
            .iter()
            .map(|assertion| assertion.to_assertion_results(test_file_stack, self, test));

        let assertions = match futures::future::try_join_all(assertions).await {
            Ok(x) => x,
            Err(err) => {
                test.error_mut(err);
                return test.finished_state();
            }
        };

        let passed = assertions.iter().all(|assertion| assertion.passed());

        let state = if passed {
            FinishedState::Success { assertions }
        } else {
            FinishedState::Failed { assertions }
        };

        test.state = TestState::Finished {
            run_duration,
            review_duration: start_time.elapsed(),
            state: state.clone(),
        };

        state
    }

    pub async fn review(
        &self,
        stack: &Stack,
        assertion: &ReviewAssertion,
        test: &Test,
    ) -> miette::Result<AssertionResult> {
        let config = stack.resolve()?;

        let harness_adapter = config.reviewer_harness_adapter();

        let review_prompt = format!(
            "Review prompt:\n{}\n\nCandidate output:\n{}",
            assertion.prompt, test.run.assistant_response
        );
        let extra_args = config
            .reviewer
            .profile
            .extra_args
            .iter()
            .map(|x| x.as_str())
            .collect::<Vec<_>>();

        let Some(workspace) = test.workspace.as_ref() else {
            unreachable!("workspace must be set at this point");
        };

        let cmd = harness_adapter
            .get_run_command(
                Some(&config.reviewer.system_prompt),
                workspace.as_path(),
                &extra_args,
                &review_prompt,
                true,
            )
            .into_miette()?;

        #[derive(Deserialize)]
        struct ReviewerResponse {
            score: f64,
            rationale: String,
        }

        let workspace_dir = self.setup_test_workspace(&test.id()).into_miette()?;

        let sandbox = config.sandbox();
        let mut review_test_run = TestRun::new();

        let extra_allowed = config
            .reviewer
            .profile
            .harness
            .extra_allowed_paths()
            .into_miette()?;

        sandbox
            .run(
                &workspace_dir,
                &extra_allowed
                    .iter()
                    .map(|x| x.as_path())
                    .collect::<Vec<_>>(),
                &cmd.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
                |command_output| {
                    // TODO basic review timeouts
                    if let Err(err) = harness_adapter
                        .parse_result(&mut review_test_run, command_output)
                        .into_miette()
                    {
                        return SandboxControl::Cancel {
                            reason: err,
                            timeout: false,
                        };
                    }
                    SandboxControl::Continue
                },
            )
            .await?;

        let model_output = &review_test_run.assistant_response.trim();
        if model_output.is_empty() {
            miette::bail!("reviewer returned empty output");
        }

        let model_output = model_output
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```JSON")
            .trim_matches('`')
            .trim();

        let response: ReviewerResponse = serde_json::from_str(model_output)
            .with_context(|| format!("failed to parse reviewer JSON response: {model_output}"))
            .into_miette()?;
        if !(0.0..=1.0).contains(&response.score) {
            miette::bail!(
                "reviewer score must be between 0 and 1, got {}",
                response.score
            );
        }

        Ok(AssertionResult::Review {
            label: format!("review: {}", assertion.prompt),
            output: test.run.assistant_response.clone(),
            score: response.score,
            passing_score: assertion
                .pass_threshold
                .unwrap_or(config.reviewer.default_pass_threshold),
            comment: response.rationale,
        })
    }
}
