pub mod assertion_impls;
pub mod cli;
pub mod config;
pub mod harness;
pub mod test;
pub mod testresult;
pub mod yaml_spec;

pub struct CommandOutput {
    pub stream: OutputStream,
    pub line: String,
}

pub struct CommandResult {
    pub exit_code: i32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum OutputStream {
    Stdout,
    Stderr,
}
