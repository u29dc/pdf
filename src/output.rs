use serde::Serialize;
use serde_json::{Value, json};

use crate::error::CommandError;
use crate::model::RunReport;
use crate::tool_registry::{ToolCatalogPayload, ToolDetailPayload};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Toon,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvelopeMeta {
    tool: String,
    elapsed: u64,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthPayload {
    pub status: &'static str,
    pub checks: Vec<HealthCheck>,
    pub summary: HealthSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub name: &'static str,
    pub status: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthSummary {
    pub ready: usize,
    pub degraded: usize,
    pub blocked: usize,
}

pub fn health_payload() -> HealthPayload {
    let mut checks = vec![HealthCheck {
        name: "pdf_parser",
        status: "ready",
        message: "In-process PDF parsing is available.".to_string(),
        fix: None,
        details: None,
    }];

    match which::which("qpdf") {
        Ok(path) => checks.push(HealthCheck {
            name: "qpdf",
            status: "ready",
            message: "qpdf is available for size estimation and apply-mode optimization.".to_string(),
            fix: None,
            details: Some(json!({ "path": path.display().to_string() })),
        }),
        Err(_) => checks.push(HealthCheck {
            name: "qpdf",
            status: "degraded",
            message:
                "qpdf is not available; optimize --estimate-size and optimize --apply cannot run size optimization."
                    .to_string(),
            fix: Some("Install qpdf and ensure it is on PATH."),
            details: None,
        }),
    }

    let summary = HealthSummary {
        ready: checks.iter().filter(|check| check.status == "ready").count(),
        degraded: checks.iter().filter(|check| check.status == "degraded").count(),
        blocked: checks.iter().filter(|check| check.status == "blocked").count(),
    };
    let status = if summary.blocked > 0 {
        "blocked"
    } else if summary.degraded > 0 {
        "degraded"
    } else {
        "ready"
    };

    HealthPayload {
        status,
        checks,
        summary,
    }
}

pub fn emit_tools_catalog(payload: &ToolCatalogPayload, elapsed: u128, format: OutputFormat) {
    emit_success(
        payload,
        EnvelopeMeta {
            tool: "pdf.tools".to_string(),
            elapsed: clamp_elapsed(elapsed),
            count: Some(payload.tools.len()),
            total: Some(payload.tools.len()),
            has_more: Some(false),
        },
        format,
    );
}

pub fn emit_tool_detail(payload: &ToolDetailPayload, elapsed: u128, format: OutputFormat) {
    emit_success(
        payload,
        EnvelopeMeta {
            tool: "pdf.tools".to_string(),
            elapsed: clamp_elapsed(elapsed),
            count: Some(1),
            total: Some(1),
            has_more: Some(false),
        },
        format,
    );
}

pub fn emit_health(payload: &HealthPayload, elapsed: u128, format: OutputFormat) {
    emit_success(
        payload,
        EnvelopeMeta {
            tool: "pdf.health".to_string(),
            elapsed: clamp_elapsed(elapsed),
            count: Some(payload.checks.len()),
            total: Some(payload.checks.len()),
            has_more: Some(false),
        },
        format,
    );
}

pub fn emit_optimize(report: &RunReport, elapsed: u128, format: OutputFormat) {
    emit_success(
        report,
        EnvelopeMeta {
            tool: "pdf.optimize".to_string(),
            elapsed: clamp_elapsed(elapsed),
            count: Some(report.files.len()),
            total: Some(report.summary.total),
            has_more: Some(false),
        },
        format,
    );
}

pub fn emit_command_error(tool: &str, error: &CommandError, elapsed: u128, format: OutputFormat) {
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
            elapsed: clamp_elapsed(elapsed),
            count: None,
            total: None,
            has_more: None,
        },
    };
    emit_envelope(&envelope, format, tool, elapsed);
}

fn emit_success<T: Serialize>(data: T, meta: EnvelopeMeta, format: OutputFormat) {
    let tool = meta.tool.clone();
    let elapsed = meta.elapsed;
    let envelope = SuccessEnvelope { ok: true, data, meta };
    emit_envelope(&envelope, format, &tool, u128::from(elapsed));
}

fn emit_envelope<T: Serialize>(envelope: &T, format: OutputFormat, tool: &str, elapsed: u128) {
    let rendered = match format {
        OutputFormat::Json => serde_json::to_string(envelope).map_err(|err| err.to_string()),
        OutputFormat::Toon => toon_format::encode_default(envelope).map_err(|err| err.to_string()),
    };

    match rendered {
        Ok(payload) => println!("{payload}"),
        Err(_) => emit_serialization_error(tool, elapsed, format),
    }
}

fn emit_serialization_error(tool: &str, elapsed: u128, format: OutputFormat) {
    let envelope = ErrorEnvelope {
        ok: false,
        error: ErrorBody {
            code: "serialization_error".to_string(),
            message: "failed to serialize envelope".to_string(),
            hint: "Retry the command after reducing output size.".to_string(),
            details: None,
        },
        meta: EnvelopeMeta {
            tool: tool.to_string(),
            elapsed: clamp_elapsed(elapsed),
            count: None,
            total: None,
            has_more: None,
        },
    };

    let rendered = match format {
        OutputFormat::Json => serde_json::to_string(&envelope).ok(),
        OutputFormat::Toon => toon_format::encode_default(&envelope).ok(),
    };
    if let Some(payload) = rendered {
        println!("{payload}");
    } else {
        println!(
            "{{\"ok\":false,\"error\":{{\"code\":\"serialization_error\",\"message\":\"failed to serialize error envelope\",\"hint\":\"Retry the command after reducing output size.\"}},\"meta\":{{\"tool\":\"{tool}\",\"elapsed\":{}}}}}",
            clamp_elapsed(elapsed)
        );
    }
}

fn clamp_elapsed(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
