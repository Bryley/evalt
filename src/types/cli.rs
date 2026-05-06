use std::path::PathBuf;

use clap::{
    ColorChoice, Parser, Subcommand,
    builder::{Styles, styling::AnsiColor},
};

/// Run portable YAML-defined AI evals against agent/model harnesses.
#[derive(Debug, Parser)]
#[command(name = "evalt")]
#[command(version, about, long_about = None)]
#[command(color = ColorChoice::Auto)]
#[command(styles = help_styles())]
pub struct Cli {
    /// Print machine-readable JSON output.
    #[arg(long)]
    pub json: bool,

    /// Command to run.
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Output the AI skill, designed for AI to run this command
    Ai {
        /// Outputs the schema
        #[arg(long, conflicts_with = "config_schema")]
        schema: bool,

        /// Outputs the schema for the config layout
        #[arg(long)]
        config_schema: bool,
    },
    /// Find eval files and validate that they can be parsed.
    Check {
        /// File or directory to check. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Find eval files, validate them, and run them.
    Run {
        /// File or directory to run. Defaults to the current directory.
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// Override the profile to run the tests with
        #[arg(long)]
        profile: Option<String>,

        /// Override the reviewer profile to run the tests with
        #[arg(long)]
        reviewer_profile: Option<String>,

        /// Test name filters. Runs tests whose names contain any selector.
        selectors: Vec<String>,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

fn help_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::BrightCyan.on_default().bold())
        .usage(AnsiColor::BrightGreen.on_default().bold())
        .literal(AnsiColor::BrightBlue.on_default().bold())
        .placeholder(AnsiColor::BrightYellow.on_default())
        .error(AnsiColor::BrightRed.on_default().bold())
        .valid(AnsiColor::BrightGreen.on_default())
        .invalid(AnsiColor::BrightRed.on_default())
}
