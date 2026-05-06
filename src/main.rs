use std::process::exit;

use crate::{
    output::{init_report_writer, report_writer},
    service::Service,
    types::{
        cli::{self, CliCommand},
        yaml_spec::{config_schema_json, testfile_schema_json},
    },
    utils::AnyhowExt,
};

pub mod engine;
pub mod harness;
pub mod output;
pub mod sandbox;
pub mod service;
pub mod types;
pub mod utils;

#[tokio::main]
async fn main() -> miette::Result<()> {
    let cli = cli::Cli::parse_args();
    init_report_writer(cli.json);

    match cli.command {
        CliCommand::Ai {
            schema,
            config_schema,
        } => {
            if schema {
                println!("{}", testfile_schema_json());
                return Ok(());
            }
            if config_schema {
                println!("{}", config_schema_json());
                return Ok(());
            }
            println!(include_str!("../SKILL.md"));
        }
        CliCommand::Check { path } => {
            let service = Service::new(None, None)?;
            let test_files = service.check(&path);
            let exit_code = report_writer()
                .check_files(false, &test_files)
                .into_miette()?;
            exit(exit_code);
        }
        CliCommand::Run {
            path,
            selectors,
            profile,
            reviewer_profile,
        } => {
            let service = Service::new(profile, reviewer_profile)?;

            let run_result = service.run(&path, &selectors).await?;

            let exit_code = match run_result {
                Ok(tests) => report_writer().finish_tests(&tests).into_miette()?,
                Err(failures) => {
                    let failures = failures
                        .into_iter()
                        .map(|(p, r)| (p, Err(r)))
                        .collect::<Vec<_>>();
                    report_writer().check_files(true, &failures).into_miette()?
                }
            };

            exit(exit_code);
        }
    };

    Ok(())
}
