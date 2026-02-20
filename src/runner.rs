use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::Local;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

use crate::cli::OptimizeArgs;
use crate::model::{FilePlan, RunOptions, RunOptionsView, RunReport, summarize};
use crate::pdf_ops::{analyze_file, apply_file};
use crate::scanner::collect_pdf_paths;

pub fn run_optimize(args: OptimizeArgs) -> Result<RunReport> {
    let run_options = RunOptions {
        apply: args.apply,
        estimate_size: args.estimate_size || args.apply,
        min_size_savings_bytes: args.min_size_savings_bytes,
        min_size_savings_percent: args.min_size_savings_percent,
        jobs: args.jobs,
        no_backup: args.no_backup,
    };

    if let Some(jobs) = run_options.jobs {
        let _ = ThreadPoolBuilder::new().num_threads(jobs).build_global();
    }

    let target_path = resolve_target_path(&args.path)?;
    let mut files = collect_pdf_paths(&target_path)?;
    files.sort();

    let mut plans: Vec<FilePlan> = files.par_iter().map(|path| analyze_file(path, &run_options)).collect();
    plans.sort_by(|left, right| left.path.cmp(&right.path));

    let mut backup_root_path = String::new();

    if run_options.apply {
        let changed_paths = plans
            .iter()
            .filter(|plan| plan.changed && !plan.skipped)
            .map(|plan| PathBuf::from(&plan.path))
            .collect::<Vec<PathBuf>>();

        if !run_options.no_backup && !changed_paths.is_empty() {
            let backup_root = create_backup_snapshot(&changed_paths)?;
            backup_root_path = backup_root.display().to_string();
        }

        plans.par_iter_mut().for_each(|plan| apply_file(plan, &run_options));
    }

    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let mode = if run_options.apply { "apply" } else { "plan" }.to_string();
    let summary = summarize(&plans);

    let mut report = RunReport {
        timestamp: timestamp.clone(),
        mode: mode.clone(),
        target_path: target_path.display().to_string(),
        options: RunOptionsView::from(&run_options),
        summary,
        backup_root: backup_root_path,
        report_path: String::new(),
        files: plans,
    };

    let report_path = write_report(&report, &timestamp, &mode)?;
    report.report_path = report_path.display().to_string();

    Ok(report)
}

fn resolve_target_path(input: &Path) -> Result<PathBuf> {
    if input.is_absolute() {
        return Ok(input.to_path_buf());
    }
    Ok(env::current_dir()
        .context("failed to read current directory")?
        .join(input))
}

fn tools_home() -> Result<PathBuf> {
    if let Ok(value) = env::var("TOOLS_HOME") {
        return Ok(PathBuf::from(value));
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow!("home directory is not available"))?;
    Ok(home.join(".tools"))
}

fn pdf_home() -> Result<PathBuf> {
    if let Ok(value) = env::var("PDF_HOME") {
        return Ok(PathBuf::from(value));
    }
    Ok(tools_home()?.join("pdf"))
}

fn backup_root() -> Result<PathBuf> {
    Ok(pdf_home()?.join("backups"))
}

fn reports_root() -> Result<PathBuf> {
    Ok(pdf_home()?.join("reports"))
}

fn create_backup_snapshot(paths: &[PathBuf]) -> Result<PathBuf> {
    let root = backup_root()?;
    fs::create_dir_all(&root).with_context(|| format!("failed to create backup root: {}", root.display()))?;
    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let run_backup_root = root.join(timestamp);
    fs::create_dir_all(&run_backup_root)
        .with_context(|| format!("failed to create backup directory: {}", run_backup_root.display()))?;

    for source in paths {
        let relative = source.strip_prefix("/").unwrap_or(source);
        let destination = run_backup_root.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create backup parent path: {}", parent.display()))?;
        }
        fs::copy(source, &destination).with_context(|| {
            format!(
                "failed to copy backup file: {} -> {}",
                source.display(),
                destination.display()
            )
        })?;
    }

    Ok(run_backup_root)
}

fn write_report(report: &RunReport, timestamp: &str, mode: &str) -> Result<PathBuf> {
    let root = reports_root()?;
    fs::create_dir_all(&root).with_context(|| format!("failed to create reports root: {}", root.display()))?;
    let report_path = root.join(format!("{timestamp}-{mode}.json"));

    let payload = serde_json::to_string_pretty(report).context("failed to serialize report")?;
    fs::write(&report_path, payload).with_context(|| format!("failed to write report: {}", report_path.display()))?;
    Ok(report_path)
}
