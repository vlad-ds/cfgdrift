use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = cfgdrift::cli::Cli::parse();

    match cfgdrift::run(cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(2)
        }
    }
}
