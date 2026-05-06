use crate::{
    output::ReportWriter,
    types::{test::{Test, TestState}, yaml_spec::TestFile},
    utils::miette_to_json_string,
};
use serde_json::{Value, json};

pub struct JsonWriter;

impl ReportWriter for JsonWriter {
    fn check_files(
        &self,
        _error_only: bool,
        results: &[(std::path::PathBuf, miette::Result<TestFile>)],
    ) -> anyhow::Result<i32> {
        let has_error = results.iter().any(|(_, result)| result.is_err());

        let checks = results
            .iter()
            .map(|(path, result)| {
                let mut val = json!({
                    "path": path,
                    "success": result.is_ok()
                });
                if let Err(err) = result {
                    val["err"] = miette_to_json_string(err)?;
                }
                Ok(val)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let val = json!({
            "checks": checks,
            "errored": has_error,
        });

        let Ok(raw_json) = serde_json::to_string(&val) else {
            unreachable!("Values should always be serializable");
        };

        println!("{raw_json}");

        Ok(if has_error { 1 } else { 0 })
    }

    fn update_test(&self, _test: &Test) -> anyhow::Result<()> {
        Ok(())
    }

    fn finish_tests(&self, tests: &[Test]) -> anyhow::Result<i32> {
        let total_tests = tests.len();
        let mut failed_tests = 0;
        for test in tests {
            if test.failed_test() {
                failed_tests += 1;
            }
        }

        let tests = tests
            .iter()
            .map(|t| self.finish_test(t))
            .collect::<Result<Vec<_>, _>>()?;

        let val = json!({
            "tests": tests,
            "total_tests": total_tests,
            "failed_tests": failed_tests,
            "passed_tests": (total_tests - failed_tests),
        });

        let Ok(raw_json) = serde_json::to_string(&val) else {
            unreachable!("Values should always be serializable");
        };

        println!("{raw_json}");

        Ok(if failed_tests > 1 { 1 } else { 0 })
    }
}

impl JsonWriter {
    pub fn finish_test(&self, test: &Test) -> anyhow::Result<Value> {
        let id = test.id();

        let state = match &test.state {
            TestState::Finished { run_duration, review_duration, state } => json!({
                "type": "finished",
                "run_duration": run_duration,
                "review_duration": review_duration,
                "state": state,
            }),
            // These options should not really be shown unless there is some kind of error or panic
            TestState::Pending => json!({ "type": "pending" }),
            TestState::Running { .. } => json!({ "type": "running" }),
            TestState::Reviewing { run_duration, .. } => json!({
                "type": "reviewing",
                "run_duration": run_duration,
            }),
        };

        Ok(json!({
            "path": id.path,
            "name": id.name,
            "profile": test.profile,
            "workspace": test.workspace,
            "state": state,
        }))
    }
}
