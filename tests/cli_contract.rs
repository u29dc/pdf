use std::path::Path;
use std::process::Command;

use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream, dictionary};
use serde_json::Value;
use tempfile::{Builder, TempDir};

fn run_pdf(args: &[&str], pdf_home: &TempDir) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pdf"))
        .args(args)
        .env("PDF_HOME", pdf_home.path())
        .output()
        .expect("run pdf binary")
}

fn parse_single_json_line(stdout: &[u8]) -> Value {
    let text = String::from_utf8(stdout.to_vec()).expect("stdout utf8");
    let trimmed = text.trim();
    assert_eq!(trimmed.lines().count(), 1, "stdout must contain exactly one JSON line");
    serde_json::from_str(trimmed).expect("stdout json")
}

fn write_sample_pdf(path: &Path) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
        },
    });
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 18.into()]),
            Operation::new("Td", vec![72.into(), 720.into()]),
            Operation::new("Tj", vec![Object::string_literal("hello contract")]),
            Operation::new("ET", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().expect("encode content")));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.compress();
    doc.save(path).expect("save sample pdf");
}

fn visible_tempdir(prefix: &str) -> TempDir {
    Builder::new()
        .prefix(prefix)
        .tempdir_in(std::env::temp_dir())
        .expect("visible tempdir")
}

#[test]
fn tools_catalog_is_json_first_and_discoverable() {
    let pdf_home = TempDir::new().expect("tempdir");
    let output = run_pdf(&["tools"], &pdf_home);

    assert!(output.status.success(), "tools should succeed");
    assert!(
        output.stderr.is_empty(),
        "json mode should not write diagnostic text to stderr on success"
    );

    let payload = parse_single_json_line(&output.stdout);
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(payload["meta"]["tool"], Value::String("pdf.tools".to_string()));
    assert!(payload["meta"]["elapsed"].is_u64());
    assert_eq!(
        payload["data"]["version"],
        Value::String(env!("CARGO_PKG_VERSION").to_string())
    );
    assert!(payload["data"]["globalFlags"].is_array());
    assert!(payload["data"]["tools"].is_array());

    let tools = payload["data"]["tools"].as_array().expect("tools array");
    assert_eq!(payload["meta"]["count"], Value::from(tools.len()));
    assert!(tools.iter().any(|tool| tool["name"] == "pdf.tools"));
    assert!(tools.iter().any(|tool| tool["name"] == "pdf.optimize"));

    let optimize = tools
        .iter()
        .find(|tool| tool["name"] == "pdf.optimize")
        .expect("optimize tool");
    for field in [
        "name",
        "command",
        "category",
        "description",
        "parameters",
        "outputFields",
        "outputSchema",
        "inputSchema",
        "idempotent",
        "rateLimit",
        "example",
    ] {
        assert!(optimize.get(field).is_some(), "optimize tool metadata missing {field}");
    }
}

#[test]
fn optimize_defaults_to_success_envelope_json() {
    let pdf_home = TempDir::new().expect("tempdir");
    let sample_dir = visible_tempdir("pdf-contract-");
    let sample_pdf = sample_dir.path().join("sample.pdf");
    write_sample_pdf(&sample_pdf);

    let output = run_pdf(&["optimize", sample_pdf.to_str().expect("sample path")], &pdf_home);

    assert!(output.status.success(), "optimize plan should succeed");
    assert!(
        output.stderr.is_empty(),
        "json mode should not write diagnostic text to stderr on success"
    );

    let payload = parse_single_json_line(&output.stdout);
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(payload["meta"]["tool"], Value::String("pdf.optimize".to_string()));
    assert!(payload["meta"]["elapsed"].is_u64());
    assert_eq!(payload["meta"]["count"], Value::from(1_u64));
    assert_eq!(payload["data"]["mode"], Value::String("plan".to_string()));
    assert_eq!(
        payload["data"]["targetPath"],
        Value::String(sample_pdf.display().to_string())
    );
    assert_eq!(payload["data"]["summary"]["total"], Value::from(1_u64));
    assert!(payload["data"]["reportPath"].as_str().is_some());
    assert!(payload["data"]["files"].is_array());
}

#[test]
fn optimize_errors_use_error_envelope_json() {
    let pdf_home = TempDir::new().expect("tempdir");
    let missing = pdf_home.path().join("missing.pdf");

    let output = run_pdf(&["optimize", missing.to_str().expect("missing path")], &pdf_home);

    assert_eq!(output.status.code(), Some(1));

    let payload = parse_single_json_line(&output.stdout);
    assert_eq!(payload["ok"], Value::Bool(false));
    assert_eq!(payload["meta"]["tool"], Value::String("pdf.optimize".to_string()));
    assert!(payload["meta"]["elapsed"].is_u64());
    assert_eq!(payload["error"]["code"], Value::String("target_not_found".to_string()));
    assert!(payload["error"]["message"].as_str().is_some());
    assert!(payload["error"]["hint"].as_str().is_some());
    assert_eq!(
        payload["error"]["details"]["path"],
        Value::String(missing.display().to_string())
    );
}
