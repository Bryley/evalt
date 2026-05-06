use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use miette::bail;
use serde::Serialize;

use crate::{
    output::report_writer,
    types::{
        config::{Profile, RunConfig},
        testresult::AssertionResult,
        yaml_spec::{TestCase, testfile::TestId},
    },
    utils::{AnyhowExt, serialize_miette_report},
};

#[derive(Debug, Clone)]
pub struct Test {
    /// Incremental test run aggregate information
    pub run: TestRun,
    pub state: TestState,
    /// The original defined testcase for this test
    pub test_case: TestCase,
    /// The path to the workspace that gets updated
    pub workspace: Option<PathBuf>,
    /// The profile the test is using
    pub profile: Option<Profile>,
}

impl Test {
    pub fn id(&self) -> TestId {
        self.test_case.id()
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.state, TestState::Finished { .. })
    }

    pub fn error(mut self, err: miette::Report) -> Self {
        self.error_mut(err);
        report_writer().update_test(&self).ok();
        self
    }

    pub fn timeout(mut self, err: miette::Report, during_review: bool) -> Self {
        self.timeout_mut(err, during_review);
        report_writer().update_test(&self).ok();
        self
    }

    pub fn error_mut(&mut self, err: miette::Report) {
        self.state = TestState::Finished {
            run_duration: Duration::default(),
            review_duration: Duration::default(),
            state: FinishedState::Errored {
                error: Arc::new(err),
                during_reviewing: false,
            },
        };
    }

    pub fn timeout_mut(&mut self, err: miette::Report, review: bool) {
        self.state = TestState::Finished {
            run_duration: Duration::default(),
            review_duration: Duration::default(),
            state: FinishedState::TimedOut {
                err: Arc::new(err),
                during_reviewing: review,
            },
        };
    }

    /// # Panics
    /// If the state is not Finished
    pub fn finished_state(&self) -> FinishedState {
        let TestState::Finished { state, .. } = &self.state else {
            unreachable!("state must be Finished");
        };
        state.clone()
    }

    pub fn failed_test(&self) -> bool {
        match &self.state {
            TestState::Finished { state, .. } => !matches!(state, FinishedState::Success { .. }),
            _ => false,
        }
    }

    pub fn to_running(&mut self) {
        self.state = TestState::Running {
            start_time: Instant::now(),
        };
        if let Err(err) = report_writer().update_test(self).into_miette() {
            self.error_mut(err);
        }
    }

    pub fn to_reviewing(&mut self) {
        let TestState::Running { start_time } = self.state else {
            unreachable!(
                "must be in state 'Running' to move to 'Reviewing' (had {:?})",
                self.state
            );
        };

        self.run.last_action = LlmAction::StartingUp;

        self.state = TestState::Reviewing {
            start_time: Instant::now(),
            run_duration: start_time.elapsed(),
        };
        if let Err(err) = report_writer().update_test(self).into_miette() {
            self.error_mut(err);
        };
    }

    pub fn to_finish(&mut self, finished_state: FinishedState) {
        let (run_duration, review_duration) = match self.state {
            TestState::Finished { .. } => {
                if let Err(err) = report_writer().update_test(self).into_miette() {
                    self.error_mut(err);
                };
                return;
            }
            TestState::Running { start_time } => (start_time.elapsed(), Duration::default()),
            TestState::Reviewing {
                start_time,
                run_duration,
            } => (run_duration, start_time.elapsed()),
            _ => {
                unreachable!("state must be 'Running' or 'Reviewing' to mark as finished");
            }
        };

        self.state = TestState::Finished {
            run_duration,
            review_duration,
            state: finished_state,
        };
        if let Err(err) = report_writer().update_test(self).into_miette() {
            self.error_mut(err);
        };
    }
}

impl From<TestCase> for Test {
    fn from(value: TestCase) -> Self {
        Self {
            run: TestRun::new(),
            state: TestState::Pending,
            test_case: value,
            workspace: None,
            profile: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TestState {
    /// Test exists but is waiting to be run (possibly in concurrency queue)
    Pending,
    /// Test is currently be run in a sandbox env
    Running {
        /// The exact time the test started (used to measure duration)
        start_time: Instant,
    },
    /// Test has completed and now assertions are being checked
    Reviewing {
        /// The exact time reviewing started (used to measure duration)
        start_time: Instant,
        /// Time taken to run the test (excluding review time)
        run_duration: Duration,
    },
    /// Test has completed
    Finished {
        /// Time taken to run the test (excluding review time)
        run_duration: Duration,
        /// Time it took to review
        review_duration: Duration,
        state: FinishedState,
    },
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum FinishedState {
    /// Test passed all assertions
    Success { assertions: Vec<AssertionResult> },
    /// Test failed an assertion
    Failed { assertions: Vec<AssertionResult> },
    /// Test timed out
    TimedOut {
        /// Infomation about timeout
        #[serde(serialize_with = "serialize_miette_report")]
        err: Arc<miette::Report>,
        /// If the timeout occurred during the reviewing stage or running stage
        during_reviewing: bool,
    },
    /// Running test resulted in an error
    Errored {
        /// The error report
        #[serde(serialize_with = "serialize_miette_report")]
        error: Arc<miette::Report>,
        /// If the error occurred during the reviewing stage or running stage
        during_reviewing: bool,
    },
}

#[derive(Debug, Clone)]
pub struct TestRun {
    /// When the test started
    pub started_at: Instant,

    /// The last action the test ran
    pub last_action: LlmAction,

    /// The entire thought process, or an empty string when reasoning/thinking output is unavailable.
    pub thinking_response: String,

    /// The final normalised assistant's response, or an empty string before one is available.
    pub assistant_response: String,

    /// The number of turns the agent has taken so far.
    pub turns: u64,

    /// The number of input tokens so far, or zero when unavailable.
    pub input_tokens: u64,

    /// The number of output tokens so far, or zero when unavailable.
    pub output_tokens: u64,

    /// Thinking/reasoning token count so far, or zero when unavailable.
    pub thinking_tokens: u64,

    /// Total tokens reported by harness so far, or zero when unavailable.
    pub total_tokens: u64,

    /// Total cost in millionths of a US dollar, or zero when unavailable.
    pub total_cost_micros: u64,

    /// Normalized tool calls observed in the transcript, in call order.
    pub tool_calls: Vec<ToolCall>,
}

impl TestRun {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            last_action: LlmAction::default(),
            thinking_response: String::new(),
            assistant_response: String::new(),
            turns: 0,
            input_tokens: 0,
            output_tokens: 0,
            thinking_tokens: 0,
            total_tokens: 0,
            total_cost_micros: 0,
            tool_calls: Vec::new(),
        }
    }

    /// Checks if the run result has reached anytime outs.
    /// Returns an error if the timeout was reached.
    pub fn timeout_reached(&self, run_config: &RunConfig) -> miette::Result<()> {
        let run_ms = self.started_at.elapsed().as_millis();

        if let Some(timeout_ms) = run_config.timeout_ms
            && run_ms >= timeout_ms.into()
        {
            bail!("test ran for longer than {timeout_ms}ms");
        }

        if let Some(max_turns) = run_config.max_turns
            && self.turns > max_turns
        {
            bail!("test turns exceeded {max_turns} turns");
        }

        if let Some(max_tokens) = run_config.budgets.max_tokens
            && self.total_tokens > max_tokens
        {
            bail!("test max tokens exceeded {max_tokens} tokens");
        }

        let current_cost_usd = (self.total_cost_micros as f64) / 1_000_000f64;
        if let Some(max_cost) = run_config.budgets.max_cost_usd
            && current_cost_usd > max_cost
        {
            bail!("test max cost exceeded ${max_cost:.2} USD (${current_cost_usd:.2} USD)");
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub enum LlmAction {
    #[default]
    StartingUp,
    ToolCall,
    Thinking,
    Responding,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    /// The ID assigned to the tool call by the provider, or the tool call index as a string if unavailable.
    pub id: String,

    /// Name of the tool (`bash`, `read`, `write`, so on...)
    pub name: String,

    /// The arguments given into the tool
    pub args: serde_json::Value,

    /// The tools state
    pub tool_state: ToolState,
}

#[derive(Debug, Clone)]
pub enum ToolState {
    /// Tool is still running
    Running,
    /// Tool has successfully finished
    Finished {
        /// The raw output from the tool call
        output: serde_json::Value,
        /// How long it took for the tool to run in ms (if available).
        duration_ms: Option<u64>,
        /// Whether the tool errored/failed. Failed tool calls still use this finished state.
        errored: bool,
    },
}
