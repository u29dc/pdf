mod cli;
mod model;
mod output;
mod pdf_ops;
mod runner;
mod scanner;

use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::output::{emit_error_json, emit_optimize_json, emit_tools_json, print_optimize_report, print_tools};
use crate::runner::run_optimize;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let start = Instant::now();

    match cli.command {
        Commands::Tools(args) => {
            if args.json {
                emit_tools_json(start.elapsed().as_millis());
            } else {
                print_tools();
            }
            ExitCode::SUCCESS
        }
        Commands::Optimize(args) => {
            let json_mode = args.json;
            match run_optimize(args) {
                Ok(report) => {
                    if json_mode {
                        emit_optimize_json(&report, start.elapsed().as_millis());
                    } else {
                        print_optimize_report(&report);
                    }
                    if report.mode == "apply" && report.summary.failed > 0 {
                        return ExitCode::from(3);
                    }
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    if json_mode {
                        emit_error_json(
                            "pdf.optimize",
                            "runtime_error",
                            &err.to_string(),
                            "Verify input path and permissions, then retry.",
                            start.elapsed().as_millis(),
                        );
                    } else {
                        eprintln!("ERROR: {err}");
                    }
                    ExitCode::FAILURE
                }
            }
        }
    }
}
