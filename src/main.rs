#![recursion_limit = "512"]

mod cli;
mod error;
mod model;
mod output;
mod pdf_ops;
mod runner;
mod scanner;
mod tool_registry;

use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use serde_json::json;

use crate::cli::{Cli, Commands};
use crate::error::CommandError;
use crate::model::RunReport;
use crate::output::{
    OutputMode, emit_command_error, emit_optimize_json, emit_tool_detail_json, emit_tools_catalog_json,
    print_command_error, print_optimize_report, print_tool_detail, print_tools_catalog,
};
use crate::runner::run_optimize;
use crate::tool_registry::{catalog_payload, detail_payload};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let start = Instant::now();
    let output_mode = if cli.text { OutputMode::Text } else { OutputMode::Json };

    match cli.command {
        Commands::Tools(args) => {
            if let Some(name) = args.name.as_deref() {
                match detail_payload(name) {
                    Some(payload) => {
                        match output_mode {
                            OutputMode::Json => emit_tool_detail_json(&payload, start.elapsed().as_millis()),
                            OutputMode::Text => print_tool_detail(&payload),
                        }
                        ExitCode::SUCCESS
                    }
                    None => exit_with_error(
                        "pdf.tools",
                        CommandError::failure(
                            "tool_not_found",
                            format!("unknown tool: {name}"),
                            "Run `pdf tools` to inspect available tool names.",
                        )
                        .with_details(json!({ "name": name })),
                        output_mode,
                        start.elapsed().as_millis(),
                    ),
                }
            } else {
                let payload = catalog_payload();
                match output_mode {
                    OutputMode::Json => emit_tools_catalog_json(&payload, start.elapsed().as_millis()),
                    OutputMode::Text => print_tools_catalog(&payload),
                }
                ExitCode::SUCCESS
            }
        }
        Commands::Optimize(args) => match run_optimize(args) {
            Ok(report) => {
                if report.mode == "apply" && report.summary.failed > 0 {
                    let err = apply_failure_error(&report);
                    match output_mode {
                        OutputMode::Json => emit_command_error("pdf.optimize", &err, start.elapsed().as_millis()),
                        OutputMode::Text => {
                            print_optimize_report(&report);
                            print_command_error("pdf.optimize", &err);
                        }
                    }
                    return ExitCode::from(err.exit_status().code());
                }

                match output_mode {
                    OutputMode::Json => emit_optimize_json(&report, start.elapsed().as_millis()),
                    OutputMode::Text => print_optimize_report(&report),
                }
                ExitCode::SUCCESS
            }
            Err(err) => exit_with_error("pdf.optimize", err, output_mode, start.elapsed().as_millis()),
        },
    }
}

fn exit_with_error(tool: &str, err: CommandError, output_mode: OutputMode, elapsed: u128) -> ExitCode {
    match output_mode {
        OutputMode::Json => emit_command_error(tool, &err, elapsed),
        OutputMode::Text => print_command_error(tool, &err),
    }
    ExitCode::from(err.exit_status().code())
}

fn apply_failure_error(report: &RunReport) -> CommandError {
    let failures = report
        .files
        .iter()
        .filter(|plan| !plan.apply_error.is_empty())
        .map(|plan| {
            json!({
                "path": plan.path,
                "applyError": plan.apply_error,
            })
        })
        .collect::<Vec<_>>();

    CommandError::failure(
        "apply_incomplete",
        format!("optimize apply finished with {} failed file(s)", report.summary.failed),
        "Inspect `reportPath` and retry the failed files after resolving the reported write issues.",
    )
    .with_details(json!({
        "reportPath": report.report_path,
        "failed": report.summary.failed,
        "total": report.summary.total,
        "files": failures,
    }))
}
