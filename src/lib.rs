use std::process::ExitCode;

use anyhow::Result;

pub mod cli;
pub mod diff;
pub mod ignore;
pub mod load;
pub mod output;
pub mod redact;

pub fn run(cli: cli::Cli) -> Result<ExitCode> {
    match cli.command {
        cli::Command::Diff(args) => {
            let ignore =
                ignore::IgnoreMatcher::from_sources(&args.ignore, args.ignore_file.as_deref())?;
            let report = diff::compare_paths(
                &args.baseline,
                &args.candidate,
                diff::CompareOptions {
                    redact: !args.no_redact,
                    ignore,
                },
            )?;

            let rendered = output::render_report(&report, args.format, args.summary_only)?;
            if !rendered.is_empty() {
                println!("{rendered}");
            }

            if report.has_drift() && !args.exit_zero {
                Ok(ExitCode::from(1))
            } else {
                Ok(ExitCode::SUCCESS)
            }
        }
        cli::Command::Formats => {
            println!("{}", load::supported_formats().join("\n"));
            Ok(ExitCode::SUCCESS)
        }
    }
}
