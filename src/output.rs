use serde::Serialize;

use crate::model::RunReport;

#[derive(Debug, Serialize)]
struct EnvelopeMeta {
    tool: String,
    elapsed_ms: u128,
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
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    ok: bool,
    error: ErrorBody,
    meta: EnvelopeMeta,
}

#[derive(Debug, Serialize)]
struct ToolDescriptor {
    name: &'static str,
    command: &'static str,
    description: &'static str,
}

pub fn print_tools() {
    println!("pdf tools");
    println!(
        "- optimize: pdf optimize <path> [--apply] [--estimate-size] [--json] [--min-size-savings-bytes <n>] [--min-size-savings-percent <pct>] [--jobs <n>] [--no-backup]"
    );
}

pub fn emit_tools_json(elapsed_ms: u128) {
    let data = vec![ToolDescriptor {
        name: "optimize",
        command: "pdf optimize <path>",
        description: "sanitize metadata and optimize one pdf file or one directory tree",
    }];
    emit_success_json("pdf.tools", data, elapsed_ms);
}

pub fn print_optimize_report(report: &RunReport) {
    println!(
        "[{}] scanned={} changed={} skipped={} applied={} failed={}",
        report.mode.to_uppercase(),
        report.summary.total,
        report.summary.changed,
        report.summary.skipped,
        report.summary.applied,
        report.summary.failed
    );
    println!(
        "[OPT] checked={} recommended={} est_saved={}B actual_saved={}B",
        report.summary.optimization_checked,
        report.summary.optimization_recommended,
        report.summary.estimated_saved_total_bytes,
        report.summary.actual_saved_total_bytes
    );
    println!("[REPORT] {}", report.report_path);
    if !report.backup_root.is_empty() {
        println!("[BACKUP] {}", report.backup_root);
    }
}

pub fn emit_optimize_json(report: &RunReport, elapsed_ms: u128) {
    emit_success_json("pdf.optimize", report, elapsed_ms);
}

pub fn emit_error_json(tool: &str, code: &str, message: &str, hint: &str, elapsed_ms: u128) {
    let envelope = ErrorEnvelope {
        ok: false,
        error: ErrorBody {
            code: code.to_string(),
            message: message.to_string(),
            hint: hint.to_string(),
        },
        meta: EnvelopeMeta {
            tool: tool.to_string(),
            elapsed_ms,
        },
    };
    match serde_json::to_string(&envelope) {
        Ok(payload) => println!("{payload}"),
        Err(_) => println!(
            "{{\"ok\":false,\"error\":{{\"code\":\"serialization_error\",\"message\":\"failed to serialize error envelope\",\"hint\":\"retry\"}},\"meta\":{{\"tool\":\"{tool}\",\"elapsed_ms\":{elapsed_ms}}}}}"
        ),
    }
}

fn emit_success_json<T: Serialize>(tool: &str, data: T, elapsed_ms: u128) {
    let envelope = SuccessEnvelope {
        ok: true,
        data,
        meta: EnvelopeMeta {
            tool: tool.to_string(),
            elapsed_ms,
        },
    };
    match serde_json::to_string(&envelope) {
        Ok(payload) => println!("{payload}"),
        Err(_) => println!(
            "{{\"ok\":false,\"error\":{{\"code\":\"serialization_error\",\"message\":\"failed to serialize success envelope\",\"hint\":\"retry\"}},\"meta\":{{\"tool\":\"{tool}\",\"elapsed_ms\":{elapsed_ms}}}}}"
        ),
    }
}
