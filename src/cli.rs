use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "pdf", version, about = "Personal PDF utility")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// List available tool commands.
    Tools(ToolsArgs),
    /// Sanitize and optimize PDFs for one file or one directory tree.
    Optimize(OptimizeArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ToolsArgs {
    /// Emit JSON output envelope.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Args)]
pub struct OptimizeArgs {
    /// PDF file or directory path.
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Apply changes in place. Without this flag, run is read-only.
    #[arg(long)]
    pub apply: bool,

    /// Emit JSON output envelope.
    #[arg(long)]
    pub json: bool,

    /// Estimate per-file size savings during planning.
    #[arg(long)]
    pub estimate_size: bool,

    /// Minimum byte savings required to keep optimization output.
    #[arg(long, default_value_t = 1024)]
    pub min_size_savings_bytes: u64,

    /// Minimum percentage savings required to keep optimization output.
    #[arg(long, default_value_t = 0.5)]
    pub min_size_savings_percent: f64,

    /// Number of worker threads. Defaults to CPU core count.
    #[arg(long, value_parser = parse_jobs)]
    pub jobs: Option<usize>,

    /// Skip backup creation in apply mode.
    #[arg(long)]
    pub no_backup: bool,
}

fn parse_jobs(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("invalid --jobs value: {value}"))?;
    if parsed == 0 {
        return Err("--jobs must be at least 1".to_string());
    }
    Ok(parsed)
}
