use serde::Serialize;
use serde_json::Value;

use crate::error::CommandError;
use crate::model::RunReport;
use crate::tool_registry::{ToolCatalogPayload, ToolDescriptor, ToolDetailPayload};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Json,
    Text,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvelopeMeta {
    tool: String,
    elapsed: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    has_more: Option<bool>,
}

#[derive(Debug, Serialize)]
struct SuccessEnvelope<T: Serialize> {
    ok: bool,
    data: T,
    meta: EnvelopeMeta,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: String,
    message: String,
    hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    ok: bool,
    error: ErrorBody,
    meta: EnvelopeMeta,
}

pub fn print_tools_catalog(payload: &ToolCatalogPayload) {
    println!("pdf {}", payload.version);
    println!("global flags:");
    for flag in &payload.global_flags {
        println!(
            "  {} ({}) default={} {}",
            flag.name, flag.value_type, flag.default, flag.description
        );
    }
    println!("tools:");
    for tool in &payload.tools {
        print_tool_summary(tool);
    }
}

pub fn print_tool_detail(payload: &ToolDetailPayload) {
    println!("pdf {}", payload.version);
    println!("global flags:");
    for flag in &payload.global_flags {
        println!(
            "  {} ({}) default={} {}",
            flag.name, flag.value_type, flag.default, flag.description
        );
    }
    println!("tool:");
    print_tool_summary(&payload.tool);
    println!("  parameters:");
    for parameter in &payload.tool.parameters {
        let required = if parameter.required { "required" } else { "optional" };
        println!(
            "    {} ({}, {}) {}",
            parameter.name, parameter.value_type, required, parameter.description
        );
    }
    println!("  output fields:");
    for field in &payload.tool.output_fields {
        println!("    {} ({}) {}", field.name, field.value_type, field.description);
    }
    println!(
        "  example: {} {}",
        payload.tool.example.command, payload.tool.example.description
    );
}

pub fn emit_tools_catalog_json(payload: &ToolCatalogPayload, elapsed: u128) {
    emit_success_json(
        "pdf.tools",
        payload,
        EnvelopeMeta {
            tool: "pdf.tools".to_string(),
            elapsed,
            count: Some(payload.tools.len()),
            total: Some(payload.tools.len()),
            has_more: Some(false),
        },
    );
}

pub fn emit_tool_detail_json(payload: &ToolDetailPayload, elapsed: u128) {
    emit_success_json(
        "pdf.tools",
        payload,
        EnvelopeMeta {
            tool: "pdf.tools".to_string(),
            elapsed,
            count: Some(1),
            total: Some(1),
            has_more: Some(false),
        },
    );
}

pub fn print_optimize_report(report: &RunReport) {
    let mode_label = format!("[{}]", report.mode.to_uppercase());
    let est_saved_mb = bytes_to_mb(report.summary.estimated_saved_total_bytes);
    let actual_saved_mb = bytes_to_mb(report.summary.actual_saved_total_bytes);

    print_status_line(
        &mode_label,
        &format!(
            "scanned={} changed={} skipped={} applied={} failed={}",
            report.summary.total,
            report.summary.changed,
            report.summary.skipped,
            report.summary.applied,
            report.summary.failed
        ),
    );
    print_status_line(
        "[OPT]",
        &format!(
            "checked={} recommended={} est_saved={:.2}MB actual_saved={:.2}MB",
            report.summary.optimization_checked, report.summary.optimization_recommended, est_saved_mb, actual_saved_mb
        ),
    );
    print_status_line("[REPORT]", &report.report_path);
    if !report.backup_root.is_empty() {
        print_status_line("[BACKUP]", &report.backup_root);
    }
}

pub fn emit_optimize_json(report: &RunReport, elapsed: u128) {
    emit_success_json(
        "pdf.optimize",
        report,
        EnvelopeMeta {
            tool: "pdf.optimize".to_string(),
            elapsed,
            count: Some(report.files.len()),
            total: Some(report.summary.total),
            has_more: Some(false),
        },
    );
}

pub fn emit_command_error(tool: &str, error: &CommandError, elapsed: u128) {
    let envelope = ErrorEnvelope {
        ok: false,
        error: ErrorBody {
            code: error.code().to_string(),
            message: error.message().to_string(),
            hint: error.hint().to_string(),
            details: error.details().cloned(),
        },
        meta: EnvelopeMeta {
            tool: tool.to_string(),
            elapsed,
            count: None,
            total: None,
            has_more: None,
        },
    };
    match serde_json::to_string(&envelope) {
        Ok(payload) => println!("{payload}"),
        Err(_) => println!(
            "{{\"ok\":false,\"error\":{{\"code\":\"serialization_error\",\"message\":\"failed to serialize error envelope\",\"hint\":\"Retry the command after reducing output size.\"}},\"meta\":{{\"tool\":\"{tool}\",\"elapsed\":{elapsed}}}}}"
        ),
    }
}

pub fn print_command_error(tool: &str, error: &CommandError) {
    eprintln!("ERROR [{tool}] {}: {}", error.code(), error.message());
    eprintln!("HINT  {}", error.hint());
    if let Some(details) = error.details() {
        eprintln!("DETAILS {details}");
    }
}

fn emit_success_json<T: Serialize>(tool: &str, data: T, meta: EnvelopeMeta) {
    let envelope = SuccessEnvelope { ok: true, data, meta };
    match serde_json::to_string(&envelope) {
        Ok(payload) => println!("{payload}"),
        Err(_) => println!(
            "{{\"ok\":false,\"error\":{{\"code\":\"serialization_error\",\"message\":\"failed to serialize success envelope\",\"hint\":\"Retry the command after reducing output size.\"}},\"meta\":{{\"tool\":\"{tool}\",\"elapsed\":0}}}}"
        ),
    }
}

fn print_status_line(label: &str, message: &str) {
    const LABEL_WIDTH: usize = 8;
    println!("{label:<LABEL_WIDTH$} {message}");
}

fn bytes_to_mb(value: i64) -> f64 {
    value as f64 / 1_000_000.0
}

fn print_tool_summary(tool: &ToolDescriptor) {
    println!("  {} [{}]", tool.name, tool.category);
    println!("    {}", tool.description);
    println!("    {}", tool.command);
}
