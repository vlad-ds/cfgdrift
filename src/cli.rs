use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "cfgdrift",
    version,
    about = "Detect semantic drift between config files or config directories.",
    long_about = "cfgdrift compares JSON, YAML, TOML, and .env files structurally so CI reviews show the settings that changed, not noisy text churn."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Compare two files or two directories.
    Diff(DiffArgs),
    /// Print the supported config formats.
    Formats,
}

#[derive(Debug, Args)]
pub struct DiffArgs {
    /// Baseline file or directory.
    pub baseline: PathBuf,
    /// Candidate file or directory.
    pub candidate: PathBuf,
    /// Output format.
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
    /// Ignore a file, location, or field path. Can be repeated.
    #[arg(long, value_name = "PATTERN")]
    pub ignore: Vec<String>,
    /// Read ignore patterns from a file. Blank lines and # comments are ignored.
    #[arg(long, value_name = "PATH")]
    pub ignore_file: Option<PathBuf>,
    /// Show sensitive values instead of replacing them with <redacted>.
    #[arg(long)]
    pub no_redact: bool,
    /// Print only aggregate counts.
    #[arg(long)]
    pub summary_only: bool,
    /// Always exit 0, even when drift is found.
    #[arg(long)]
    pub exit_zero: bool,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
    Markdown,
}
