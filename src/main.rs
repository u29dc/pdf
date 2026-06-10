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

use clap::{CommandFactory, Parser};
use serde_json::json;

use crate::cli::{Cli, Commands};
use crate::error::CommandError;
use crate::model::RunReport;
use crate::output::{
    OutputFormat, emit_command_error, emit_health, emit_optimize, emit_tool_detail, emit_tools_catalog, health_payload,
};
use crate::runner::run_optimize;
use crate::tool_registry::{catalog_payload, detail_payload};

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.command.is_none() {
        print_root_help();
        return ExitCode::SUCCESS;
    }

    let start = Instant::now();
    let output_format = if cli.toon {
        OutputFormat::Toon
    } else {
        OutputFormat::Json
    };

    match cli.command.expect("command should exist") {
        Commands::Tools(args) => {
            if let Some(name) = args.name.as_deref() {
                match detail_payload(name) {
                    Some(payload) => {
                        emit_tool_detail(&payload, start.elapsed().as_millis(), output_format);
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
                        output_format,
                        start.elapsed().as_millis(),
                    ),
                }
            } else {
                let payload = catalog_payload();
                emit_tools_catalog(&payload, start.elapsed().as_millis(), output_format);
                ExitCode::SUCCESS
            }
        }
        Commands::Health => {
            let payload = health_payload();
            emit_health(&payload, start.elapsed().as_millis(), output_format);
            ExitCode::SUCCESS
        }
        Commands::Optimize(args) => match run_optimize(args) {
            Ok(report) => {
                if report.mode == "apply" && report.summary.failed > 0 {
                    let err = apply_failure_error(&report);
                    emit_command_error("pdf.optimize", &err, start.elapsed().as_millis(), output_format);
                    return ExitCode::from(err.exit_status().code());
                }

                emit_optimize(&report, start.elapsed().as_millis(), output_format);
                ExitCode::SUCCESS
            }
            Err(err) => exit_with_error("pdf.optimize", err, output_format, start.elapsed().as_millis()),
        },
    }
}

fn exit_with_error(tool: &str, err: CommandError, output_format: OutputFormat, elapsed: u128) -> ExitCode {
    emit_command_error(tool, &err, elapsed, output_format);
    ExitCode::from(err.exit_status().code())
}

fn print_root_help() {
    let mut command = Cli::command().subcommand_required(true);
    command.print_help().expect("print root help");
    println!();
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
