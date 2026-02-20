use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use lopdf::{Dictionary, Document, Object};
use tempfile::Builder;

use crate::model::{
    ACTION_OPTIMIZE, ACTION_REMOVE_XMP, ACTION_SET_TITLE, ACTION_SET_VERSION, ACTION_STRIP_DOCINFO, FilePlan,
    KEEP_DOCINFO_FIELDS, RunOptions, TARGET_PDF_VERSION, is_metadata_action,
};

enum RewriteMode {
    MetadataOnly,
    Optimized,
}

pub fn analyze_file(path: &Path, options: &RunOptions) -> FilePlan {
    let mut plan = FilePlan::new(
        path.display().to_string(),
        fs::metadata(path).map(|m| m.len()).unwrap_or(0),
    );

    let file_bytes = match fs::read(path) {
        Ok(value) => value,
        Err(err) => {
            plan.skipped = true;
            plan.skip_reason = format!("read error: {err}");
            return plan;
        }
    };
    plan.size_bytes = u64::try_from(file_bytes.len()).unwrap_or(u64::MAX);

    if contains_signature_tokens(&file_bytes) {
        plan.signed = true;
        plan.skipped = true;
        plan.skip_reason = "signed-pdf (byte-range-signature-token)".to_string();
        return plan;
    }

    let doc = match Document::load_mem(&file_bytes) {
        Ok(value) => value,
        Err(err) => {
            let detail = err.to_string();
            if is_password_error(&detail) {
                plan.password_protected = true;
                plan.skipped = true;
                plan.skip_reason = "password-protected".to_string();
            } else {
                plan.skipped = true;
                plan.skip_reason = format!("open error: {detail}");
            }
            return plan;
        }
    };

    inspect_metadata(&doc, path, &mut plan);

    if options.estimate_size {
        plan.optimization_checked = true;
        match estimate_optimized_output(path, options.apply) {
            Ok((estimated_after, staged_optimized_path)) => {
                let before = i64::try_from(plan.size_bytes).unwrap_or(i64::MAX);
                let after = i64::try_from(estimated_after).unwrap_or(i64::MAX);
                let saved = before - after;
                let saved_pct = percent(saved, plan.size_bytes);
                plan.estimated_size_after_bytes = Some(estimated_after);
                plan.estimated_saved_bytes = Some(saved);
                plan.estimated_saved_percent = Some(saved_pct);
                plan.optimization_recommended =
                    saved >= options.min_size_savings_bytes as i64 && saved_pct >= options.min_size_savings_percent;
                if plan.optimization_recommended {
                    plan.planned_actions.push(ACTION_OPTIMIZE.to_string());
                }
                if let Some(staged_path) = staged_optimized_path {
                    plan.staged_optimized_path = Some(staged_path.display().to_string());
                }
            }
            Err(err) => {
                plan.optimization_error = err.to_string();
            }
        }
    } else {
        plan.optimization_recommended = true;
        plan.planned_actions.push(ACTION_OPTIMIZE.to_string());
    }

    plan.changed = !plan.planned_actions.is_empty();
    plan
}

pub fn apply_file(plan: &mut FilePlan, options: &RunOptions) {
    if !options.apply || plan.skipped || !plan.changed || !plan.apply_error.is_empty() {
        return;
    }

    let path = PathBuf::from(&plan.path);
    let metadata_needed = plan.planned_actions.iter().any(|action| is_metadata_action(action));

    let optimized_tmp = match resolve_optimized_temp(plan, &path) {
        Ok(value) => value,
        Err(err) => {
            plan.apply_error = format!("rewrite error: {err}");
            return;
        }
    };

    let optimized_size = fs::metadata(&optimized_tmp).map(|m| m.len()).unwrap_or(plan.size_bytes);
    let optimized_saved =
        i64::try_from(plan.size_bytes).unwrap_or(i64::MAX) - i64::try_from(optimized_size).unwrap_or(i64::MAX);
    let optimized_saved_pct = percent(optimized_saved, plan.size_bytes);
    let meets_threshold = optimized_saved >= options.min_size_savings_bytes as i64
        && optimized_saved_pct >= options.min_size_savings_percent;

    if meets_threshold {
        if let Err(err) = replace_file(&optimized_tmp, &path) {
            let _ = fs::remove_file(&optimized_tmp);
            plan.apply_error = format!("replace error: {err}");
            return;
        }

        plan.applied = true;
        plan.size_after_bytes = Some(optimized_size);
        plan.actual_saved_bytes = Some(optimized_saved);
        plan.actual_saved_percent = Some(optimized_saved_pct);
        return;
    }

    if metadata_needed {
        let metadata_tmp = match create_temp_pdf_path(&path, "metadata") {
            Ok(value) => value,
            Err(err) => {
                let _ = fs::remove_file(&optimized_tmp);
                plan.apply_error = format!("temp error: {err}");
                return;
            }
        };

        if let Err(err) = rewrite_pdf(&path, &metadata_tmp, RewriteMode::MetadataOnly) {
            let _ = fs::remove_file(&optimized_tmp);
            let _ = fs::remove_file(&metadata_tmp);
            plan.apply_error = format!("metadata rewrite error: {err}");
            return;
        }

        let metadata_size = fs::metadata(&metadata_tmp).map(|m| m.len()).unwrap_or(plan.size_bytes);
        let metadata_saved =
            i64::try_from(plan.size_bytes).unwrap_or(i64::MAX) - i64::try_from(metadata_size).unwrap_or(i64::MAX);
        let metadata_saved_pct = percent(metadata_saved, plan.size_bytes);

        if let Err(err) = replace_file(&metadata_tmp, &path) {
            let _ = fs::remove_file(&optimized_tmp);
            let _ = fs::remove_file(&metadata_tmp);
            plan.apply_error = format!("replace error: {err}");
            return;
        }

        let _ = fs::remove_file(&optimized_tmp);

        plan.applied = true;
        plan.apply_note = format!(
            "optimization below threshold; metadata-only write applied (saved={}B, {:+.2}%)",
            optimized_saved, optimized_saved_pct
        );
        plan.size_after_bytes = Some(metadata_size);
        plan.actual_saved_bytes = Some(metadata_saved);
        plan.actual_saved_percent = Some(metadata_saved_pct);
        return;
    }

    let _ = fs::remove_file(&optimized_tmp);
    plan.apply_note = format!(
        "optimization below threshold; skipped write (saved={}B, {:+.2}%)",
        optimized_saved, optimized_saved_pct
    );
}

fn inspect_metadata(doc: &Document, path: &Path, plan: &mut FilePlan) {
    plan.version_before = doc.version.clone();
    plan.title_after = path
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();

    if let Ok(info_obj) = doc.trailer.get(b"Info") {
        let info_dict: Option<&Dictionary> = match info_obj {
            Object::Reference(id) => doc.get_dictionary(*id).ok(),
            Object::Dictionary(dict) => Some(dict),
            _ => None,
        };

        if let Some(dict) = info_dict {
            for (raw_key, value) in dict.iter() {
                let key = String::from_utf8_lossy(raw_key).to_string();
                if key == "Title" {
                    plan.title_before = object_to_text(value);
                }
                if !KEEP_DOCINFO_FIELDS.contains(&key.as_str()) {
                    plan.fields_to_strip.push(format!("/{key}"));
                }
            }
        }
    }

    plan.xmp_present = doc.catalog().map(|catalog| catalog.has(b"Metadata")).unwrap_or(false);

    if plan.title_before != plan.title_after {
        plan.planned_actions.push(ACTION_SET_TITLE.to_string());
    }
    if !plan.fields_to_strip.is_empty() {
        plan.planned_actions.push(ACTION_STRIP_DOCINFO.to_string());
    }
    if plan.xmp_present {
        plan.planned_actions.push(ACTION_REMOVE_XMP.to_string());
    }
    if plan.version_before != TARGET_PDF_VERSION {
        plan.planned_actions.push(ACTION_SET_VERSION.to_string());
    }
}

fn estimate_optimized_output(path: &Path, keep_artifact: bool) -> Result<(u64, Option<PathBuf>)> {
    let temp_path = create_temp_pdf_path(path, "estimate")?;
    if let Err(err) = rewrite_pdf(path, &temp_path, RewriteMode::Optimized) {
        let _ = fs::remove_file(&temp_path);
        return Err(err);
    }
    let size_after = fs::metadata(&temp_path)
        .map(|meta| meta.len())
        .with_context(|| format!("failed to read temp output metadata: {}", temp_path.display()))?;
    if keep_artifact {
        return Ok((size_after, Some(temp_path)));
    }
    let _ = fs::remove_file(&temp_path);
    Ok((size_after, None))
}

fn resolve_optimized_temp(plan: &mut FilePlan, source_path: &Path) -> Result<PathBuf> {
    if let Some(staged_path) = plan.staged_optimized_path.take() {
        let staged = PathBuf::from(staged_path);
        if staged.exists() {
            return Ok(staged);
        }
    }

    let optimized_tmp = create_temp_pdf_path(source_path, "optimized")?;
    if let Err(err) = rewrite_pdf(source_path, &optimized_tmp, RewriteMode::Optimized) {
        let _ = fs::remove_file(&optimized_tmp);
        return Err(err);
    }
    Ok(optimized_tmp)
}

fn rewrite_pdf(source_path: &Path, output_path: &Path, mode: RewriteMode) -> Result<()> {
    match mode {
        RewriteMode::MetadataOnly => {
            let mut doc = Document::load(source_path)
                .with_context(|| format!("failed to load pdf: {}", source_path.display()))?;
            apply_metadata_cleanup(&mut doc, source_path)?;
            doc.version = TARGET_PDF_VERSION.to_string();

            let output_file = File::create(output_path)
                .with_context(|| format!("failed to create output file: {}", output_path.display()))?;
            let mut writer = BufWriter::new(output_file);
            doc.save_to(&mut writer)
                .with_context(|| format!("failed to save metadata rewrite: {}", output_path.display()))?;
            writer.flush()?;
        }
        RewriteMode::Optimized => {
            let metadata_tmp = create_temp_pdf_path(source_path, "metadata-stage")?;
            let rewrite_result = rewrite_pdf(source_path, &metadata_tmp, RewriteMode::MetadataOnly);
            if let Err(err) = rewrite_result {
                let _ = fs::remove_file(&metadata_tmp);
                return Err(err);
            }

            let optimize_result = optimize_with_qpdf(&metadata_tmp, output_path);
            let _ = fs::remove_file(&metadata_tmp);
            optimize_result?;
        }
    }
    Ok(())
}

fn apply_metadata_cleanup(doc: &mut Document, source_path: &Path) -> Result<()> {
    let title = source_path
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "document".to_string());

    let old_info_ref = doc.trailer.get(b"Info").and_then(Object::as_reference).ok();
    let mut new_info_dict = Dictionary::new();

    if let Ok(info_obj) = doc.trailer.get(b"Info") {
        let info_dict: Option<&Dictionary> = match info_obj {
            Object::Reference(id) => doc.get_dictionary(*id).ok(),
            Object::Dictionary(dict) => Some(dict),
            _ => None,
        };
        if let Some(dict) = info_dict {
            for key in KEEP_DOCINFO_FIELDS {
                if let Ok(value) = dict.get(key.as_bytes()) {
                    new_info_dict.set(key.as_bytes().to_vec(), value.clone());
                }
            }
        }
    }

    new_info_dict.set("Title", Object::string_literal(title.into_bytes()));

    let new_info_id = doc.new_object_id();
    doc.objects.insert(new_info_id, Object::Dictionary(new_info_dict));
    doc.trailer.set("Info", Object::Reference(new_info_id));

    if let Some(old_id) = old_info_ref
        && old_id != new_info_id
    {
        doc.objects.remove(&old_id);
    }

    let metadata_ref = if let Ok(catalog_ref) = doc.trailer.get(b"Root").and_then(Object::as_reference) {
        if let Ok(catalog) = doc.get_dictionary_mut(catalog_ref) {
            catalog
                .remove(b"Metadata")
                .and_then(|object| object.as_reference().ok())
        } else {
            None
        }
    } else {
        None
    };

    if let Some(id) = metadata_ref {
        doc.objects.remove(&id);
    }

    Ok(())
}

fn replace_file(temp_path: &Path, target_path: &Path) -> Result<()> {
    fs::rename(temp_path, target_path).with_context(|| {
        format!(
            "failed to replace target with temp file: {} -> {}",
            temp_path.display(),
            target_path.display()
        )
    })?;
    Ok(())
}

fn create_temp_pdf_path(target: &Path, label: &str) -> Result<PathBuf> {
    let parent = target
        .parent()
        .ok_or_else(|| anyhow!("target has no parent directory: {}", target.display()))?;

    let named = Builder::new()
        .prefix(&format!(".pdf-{label}-"))
        .suffix(".tmp.pdf")
        .tempfile_in(parent)
        .with_context(|| format!("failed to create temp file in {}", parent.display()))?;

    let (_file, path) = named.keep()?;
    Ok(path)
}

fn optimize_with_qpdf(input_path: &Path, output_path: &Path) -> Result<()> {
    let qpdf_bin = which::which("qpdf").context("qpdf not found; install qpdf for optimization")?;

    let optimize = Command::new(&qpdf_bin)
        .arg("--object-streams=generate")
        .arg("--compress-streams=y")
        .arg("--recompress-flate")
        .arg("--compression-level=9")
        .arg(input_path)
        .arg(output_path)
        .output()
        .with_context(|| format!("failed to execute qpdf optimize for {}", input_path.display()))?;

    if !optimize.status.success() {
        let stderr = String::from_utf8_lossy(&optimize.stderr).to_string();
        let stdout = String::from_utf8_lossy(&optimize.stdout).to_string();
        return Err(anyhow!(
            "qpdf optimize failed for {}: {}{}",
            input_path.display(),
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!(" | {}", stdout.trim())
            }
        ));
    }

    let check = Command::new(&qpdf_bin)
        .arg("--check")
        .arg(output_path)
        .output()
        .with_context(|| format!("failed to execute qpdf check for {}", output_path.display()))?;

    let check_code = check.status.code().unwrap_or(1);
    if !(check_code == 0 || check_code == 3) {
        let stderr = String::from_utf8_lossy(&check.stderr).to_string();
        let stdout = String::from_utf8_lossy(&check.stdout).to_string();
        return Err(anyhow!(
            "qpdf output validation failed for {}: {}{}",
            output_path.display(),
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!(" | {}", stdout.trim())
            }
        ));
    }

    Ok(())
}

fn object_to_text(value: &Object) -> String {
    if let Ok(text) = value.as_str() {
        return String::from_utf8_lossy(text).to_string();
    }
    if let Ok(name) = value.as_name() {
        return String::from_utf8_lossy(name).to_string();
    }
    format!("{value:?}")
}

fn contains_signature_tokens(data: &[u8]) -> bool {
    if !contains_bytes(data, b"/ByteRange") {
        return false;
    }
    [
        b"/Type/Sig".as_slice(),
        b"/Type /Sig".as_slice(),
        b"/adbe.pkcs7".as_slice(),
        b"/ETSI.CAdES".as_slice(),
    ]
    .iter()
    .any(|needle| contains_bytes(data, needle))
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|slice| slice == needle)
}

fn is_password_error(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    lowered.contains("encrypted") || lowered.contains("password")
}

fn percent(saved_bytes: i64, size_before: u64) -> f64 {
    if size_before == 0 {
        return 0.0;
    }
    (saved_bytes as f64 / size_before as f64) * 100.0
}

#[cfg(test)]
mod tests {
    use super::{contains_signature_tokens, percent};

    #[test]
    fn detects_signature_tokens() {
        let data = b"abc /ByteRange [0 1 2] /Type/Sig xyz";
        assert!(contains_signature_tokens(data));
        let no_sig = b"abc /ByteRange [0 1 2] /Type/Catalog xyz";
        assert!(!contains_signature_tokens(no_sig));
    }

    #[test]
    fn computes_percentage() {
        assert_eq!(percent(50, 100), 50.0);
        assert_eq!(percent(-10, 100), -10.0);
        assert_eq!(percent(10, 0), 0.0);
    }
}
