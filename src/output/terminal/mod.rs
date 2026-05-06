use std::{
    fmt::Write as _,
    io::{Write as _, stdout},
    sync::{Arc, Mutex},
    time::Duration,
};

use crossterm::style::Stylize as _;
use indexmap::IndexMap;
use tokio::{task::JoinHandle, time};

use crate::{
    output::{ReportWriter, terminal::drawer::Drawer},
    types::{
        test::{FinishedState, LlmAction, Test, TestState},
        testresult::PrettyDisplay,
        yaml_spec::testfile::TestId,
    }, utils::{fail, pass, render_rail_text, timeout},
};

pub mod drawer;

const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct TerminalWriter {
    tick_task: Option<JoinHandle<()>>,
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    spinner_index: usize,
    running_tests: IndexMap<TestId, Test>,
}

impl TerminalWriter {
    pub fn new() -> Self {
        let mut output = Self {
            tick_task: None,
            inner: Arc::new(Mutex::new(Inner {
                spinner_index: 0,
                running_tests: IndexMap::new(),
            })),
        };

        output.setup_tick();
        output
    }

    fn setup_tick(&mut self) {
        if self.tick_task.is_some() {
            return;
        }

        let inner = Arc::clone(&self.inner);
        self.tick_task = Some(tokio::spawn(async move {
            let mut ticker = time::interval(Duration::from_millis(200));
            loop {
                ticker.tick().await;
                if let Ok(mut inner) = inner.lock() {
                    let _ = Self::tick_inner(&mut inner);
                }
            }
        }));
    }

    fn tick_inner(inner: &mut Inner) -> anyhow::Result<()> {
        if inner.running_tests.is_empty() {
            return Ok(());
        }
        inner.spinner_index = (inner.spinner_index + 1) % SPINNER_FRAMES.len();
        Self::redraw_inner(inner)
    }

    fn spinner(spinner_index: usize) -> &'static str {
        SPINNER_FRAMES[spinner_index % SPINNER_FRAMES.len()]
    }

    fn redraw_inner(inner: &Inner) -> anyhow::Result<()> {
        if !inner.running_tests.is_empty() {
            Self::redraw_tests_running(inner.spinner_index, inner)?
        }
        Ok(())
    }

    fn redraw_tests_running(spinner_index: usize, inner: &Inner) -> anyhow::Result<()> {
        let mut drawer = Drawer::default();

        for test in inner.running_tests.values() {
            let id = test.id();
            let test_name = format!(
                "{} {} {}",
                id.path.display().to_string().dim(),
                id.name.bold(),
                test.profile
                    .as_ref()
                    .map(|x| x.name.as_str())
                    .unwrap_or_default()
                    .dim()
                    .italic(),
            );

            let status = match test.run.last_action {
                LlmAction::StartingUp => "Starting up...".italic().dim().to_string(),
                LlmAction::ToolCall => match test.run.tool_calls.last() {
                    Some(tool) => {
                        format!(
                            "{}: {}",
                            tool.name.as_str().bold(),
                            serde_json::to_string(&tool.args)?.dim()
                        )
                    }
                    None => "Calling tool...".dim().to_string(),
                },
                LlmAction::Thinking => "Thinking...".dim().to_string(),
                LlmAction::Responding => "Responding...".dim().to_string(),
            };

            let mut status_line = String::new();
            match &test.state {
                TestState::Pending => {
                    write!(&mut status_line, "{} {test_name}", "⋯".grey())?;
                }
                TestState::Running { start_time } => {
                    let elapsed = start_time.elapsed();
                    write!(
                        &mut status_line,
                        "{} {test_name} {} => {}",
                        Self::spinner(spinner_index).blue(),
                        format!("({:.2}s)", elapsed.as_secs_f64()).dim(),
                        status,
                    )?;
                }
                TestState::Reviewing {
                    start_time,
                    run_duration,
                } => {
                    let elapsed = start_time.elapsed();
                    write!(
                        &mut status_line,
                        "{} {test_name} {} {} => {}",
                        Self::spinner(spinner_index).green(),
                        format!("({:.2}s)", run_duration.as_secs_f64()).green(),
                        format!("(Reviewing: {:.2}s)", elapsed.as_secs_f64()).dim(),
                        status,
                    )?;
                }
                TestState::Finished {
                    run_duration,
                    review_duration: _,
                    state,
                } => match state {
                    FinishedState::Success { assertions: _ } => {
                        write!(
                            &mut status_line,
                            "{} {test_name} {}",
                            pass(),
                            format!("({:.2}s)", run_duration.as_secs_f64()).green(),
                        )?;
                    }
                    FinishedState::Failed { assertions: _ } => {
                        write!(
                            &mut status_line,
                            "{} {test_name} {}",
                            fail(),
                            format!("({:.2}s)", run_duration.as_secs_f64()).red(),
                        )?;
                    }
                    FinishedState::TimedOut {
                        err: _,
                        during_reviewing,
                    } => {
                        write!(
                            &mut status_line,
                            "{} {test_name} {}",
                            timeout(),
                            format!("({:.2}s)", run_duration.as_secs_f64()).yellow(),
                        )?;
                        if *during_reviewing {
                            write!(&mut status_line, " {}", "Review timeout".italic().dim())?;
                        }
                    }
                    FinishedState::Errored {
                        error: _,
                        during_reviewing,
                    } => {
                        write!(
                            &mut status_line,
                            "{} {test_name} {}",
                            fail(),
                            format!("({:.2}s)", run_duration.as_secs_f64()).red(),
                        )?;

                        if *during_reviewing {
                            write!(&mut status_line, " {}", "Review failed".italic().dim())?;
                        }
                    }
                },
            }
            drawer.add_clipped_line(&status_line);
        }

        drawer.draw()?;

        Ok(())
    }

    fn finish_test(&self, test: &Test) -> anyhow::Result<()> {
        let mut out = stdout();

        let id = test.id();
        let test_name = format!(
            "{} {} {}",
            id.path.display().to_string().dim(),
            id.name.bold(),
            test.profile
                .as_ref()
                .map(|x| x.name.as_str())
                .unwrap_or_default()
                .dim()
                .italic(),
        );

        let sub_heading = format!(
            "  => {}",
            test.workspace
                .as_ref()
                .map(|x| x.display().to_string())
                .unwrap_or("[no workspace]".into())
        )
        .dim()
        .italic();

        let TestState::Finished {
            run_duration,
            review_duration: _,
            state,
        } = &test.state
        else {
            return Ok(());
        };

        let duration = format!("({:.2}s)", run_duration.as_secs_f64());

        match state {
            FinishedState::Success { assertions: _ } => {
                writeln!(&mut out, "{} {test_name} {}", pass(), duration.green())?;
                writeln!(&mut out, "{sub_heading}")?;
            }
            FinishedState::Failed { assertions } => {
                writeln!(&mut out, "{} {test_name} {}", fail(), duration.red())?;
                writeln!(&mut out, "{sub_heading}")?;
                if let Some(desc) = &test.test_case.desc {
                    write!(&mut out, "    {}", "desc:".bold())?;
                    if desc.contains('\n') {
                        writeln!(&mut out)?;
                        let mut rendered = String::new();
                        render_rail_text(&mut rendered, desc, 2)?;
                        writeln!(out, "{rendered}")?;
                    } else {
                        writeln!(out, " {desc}")?;
                    }
                }
                for assertion in assertions {
                    writeln!(&mut out, "{}", PrettyDisplay::with_indent(assertion, 1))?;
                }
            }
            FinishedState::TimedOut {
                err,
                during_reviewing,
            } => {
                writeln!(&mut out, "{} {test_name} {}", timeout(), duration.yellow())?;
                writeln!(&mut out, "{sub_heading}")?;
                if *during_reviewing {
                    writeln!(&mut out, "  {}", "During Reviewing".dim())?;
                }
                writeln!(&mut out, "{err:?}")?;
            }
            FinishedState::Errored {
                error,
                during_reviewing,
            } => {
                writeln!(&mut out, "{} {test_name} {}", "✗".red(), duration.red())?;
                writeln!(&mut out, "{sub_heading}")?;
                if *during_reviewing {
                    writeln!(&mut out, "  {}", "During Reviewing".dim())?;
                }
                writeln!(&mut out, "{error:?}")?;
            }
        };

        Ok(())
    }
}

impl ReportWriter for TerminalWriter {
    fn check_files(
        &self,
        error_only: bool,
        results: &[(
            std::path::PathBuf,
            miette::Result<crate::types::yaml_spec::TestFile>,
        )],
    ) -> anyhow::Result<i32> {
        let mut out = stdout();

        let total = results.len();
        let passed = results.iter().filter(|(_, result)| result.is_ok()).count();
        let failed = total.saturating_sub(passed);

        if !error_only {
            writeln!(
                out,
                "Found {} evalt YAML spec{}: {} passed, {} failed",
                total.to_string().cyan().bold(),
                if total == 1 { "" } else { "s" },
                passed.to_string().green().bold(),
                if failed == 0 {
                    failed.to_string().green().bold().to_string()
                } else {
                    failed.to_string().red().bold().to_string()
                },
            )?;
        }

        if results.is_empty() {
            writeln!(
                out,
                "{}",
                "No .eval.yaml or .eval.yml files found.".yellow().bold()
            )?;
            return Ok(0);
        }

        for (path, result) in results {
            match &result {
                Ok(_) => {
                    if !error_only {
                        writeln!(
                            out,
                            "{} {}",
                            " PASSED ".black().on_green().bold(),
                            path.display().to_string().dim()
                        )?;
                    }
                }
                Err(report) => {
                    writeln!(
                        out,
                        "{} {}",
                        " FAILED ".white().on_red().bold(),
                        path.display().to_string().dim()
                    )?;

                    writeln!(out, "{report:?}")?;
                }
            }
        }

        Ok(if failed > 0 { 1 } else { 0 })
    }

    fn update_test(&self, test: &Test) -> anyhow::Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("terminal writer state mutex poisoned"))?;

        inner.running_tests.insert(test.id(), test.clone());
        Self::redraw_inner(&inner)?;

        Ok(())
    }

    fn finish_tests(&self, tests: &[Test]) -> anyhow::Result<i32> {
        let mut out = stdout();
        Drawer::default().clear()?;

        let total_tests = tests.len();
        let mut failed_tests = 0;
        for test in tests {
            if test.failed_test() {
                failed_tests += 1;
            }
            self.finish_test(test)?;
        }

        writeln!(
            &mut out,
            "\n{} tests passed, {} tests failed, {} total",
            (total_tests - failed_tests).to_string().green(),
            failed_tests.to_string().red(),
            total_tests.to_string().blue(),
        )?;

        Ok(if failed_tests > 1 { 1 } else { 0 })
    }
}

impl Drop for TerminalWriter {
    fn drop(&mut self) {
        if let Some(task) = self.tick_task.take() {
            task.abort();
        }
    }
}
