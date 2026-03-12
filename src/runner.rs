use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};
use serde_json::json;

use crate::cli::OptimizeArgs;
use crate::error::{CommandError, CommandResult};
use crate::model::{FilePlan, RunOptions, RunOptionsView, RunReport, summarize};
use crate::pdf_ops::{analyze_file, apply_file};
use crate::scanner::collect_pdf_paths;

pub fn run_optimize(args: OptimizeArgs) -> CommandResult<RunReport> {
    let run_options = RunOptions {
        apply: args.apply,
        estimate_size: args.estimate_size || args.apply,
        min_size_savings_bytes: args.min_size_savings_bytes,
        min_size_savings_percent: args.min_size_savings_percent,
        jobs: args.jobs,
        no_backup: args.no_backup,
    };

    let worker_pool = build_thread_pool(run_options.jobs)?;

    let target_path = resolve_target_path(&args.path)?;
    let mut files = collect_pdf_paths(&target_path)?;
    files.sort();

    let mut plans: Vec<FilePlan> =
        worker_pool.install(|| files.par_iter().map(|path| analyze_file(path, &run_options)).collect());
    plans.sort_by(|left, right| left.path.cmp(&right.path));

    let mut backup_root_path = String::new();

    if run_options.apply {
        let changed_paths = plans
            .iter()
            .filter(|plan| plan.changed && !plan.skipped)
            .map(|plan| PathBuf::from(&plan.path))
            .collect::<Vec<PathBuf>>();

        if !run_options.no_backup && !changed_paths.is_empty() {
            let backup_root = create_backup_snapshot(&worker_pool, &changed_paths)?;
            backup_root_path = backup_root.display().to_string();
        }

        worker_pool.install(|| plans.par_iter_mut().for_each(|plan| apply_file(plan, &run_options)));
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

fn build_thread_pool(jobs: Option<usize>) -> CommandResult<ThreadPool> {
    let mut builder = ThreadPoolBuilder::new();
    if let Some(value) = jobs {
        builder = builder.num_threads(value);
    }
    builder.build().map_err(|err| {
        CommandError::blocked(
            "worker_pool_unavailable",
            format!("failed to initialize worker pool: {err}"),
            "Reduce `--jobs` or retry on a host that can create worker threads.",
        )
        .with_details(json!({
            "jobs": jobs,
            "source": err.to_string(),
        }))
    })
}

fn resolve_target_path(input: &Path) -> CommandResult<PathBuf> {
    if input.is_absolute() {
        return Ok(input.to_path_buf());
    }
    let current_dir = env::current_dir().map_err(|err| {
        CommandError::blocked(
            "current_directory_unavailable",
            format!("failed to read current directory: {err}"),
            "Run the command from a readable directory or pass an absolute path.",
        )
        .with_details(json!({
            "path": input.display().to_string(),
            "source": err.to_string(),
        }))
    })?;
    Ok(current_dir.join(input))
}

fn tools_home() -> CommandResult<PathBuf> {
    if let Ok(value) = env::var("TOOLS_HOME") {
        return Ok(PathBuf::from(value));
    }
    let home = dirs::home_dir().ok_or_else(|| {
        CommandError::blocked(
            "home_directory_unavailable",
            "home directory is not available",
            "Set `PDF_HOME` or `TOOLS_HOME` to a writable directory before retrying.",
        )
    })?;
    Ok(home.join(".tools"))
}

fn pdf_home() -> CommandResult<PathBuf> {
    if let Ok(value) = env::var("PDF_HOME") {
        return Ok(PathBuf::from(value));
    }
    Ok(tools_home()?.join("pdf"))
}

fn backup_root() -> CommandResult<PathBuf> {
    Ok(pdf_home()?.join("backups"))
}

fn reports_root() -> CommandResult<PathBuf> {
    Ok(pdf_home()?.join("reports"))
}

fn create_backup_snapshot(pool: &ThreadPool, paths: &[PathBuf]) -> CommandResult<PathBuf> {
    let root = backup_root()?;
    fs::create_dir_all(&root).map_err(|err| {
        CommandError::blocked(
            "backup_root_unavailable",
            format!("failed to create backup root: {}", root.display()),
            "Set `PDF_HOME` or `TOOLS_HOME` to a writable directory, or rerun with `--no-backup`.",
        )
        .with_details(json!({
            "path": root.display().to_string(),
            "source": err.to_string(),
        }))
    })?;
    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let run_backup_root = root.join(timestamp);
    fs::create_dir_all(&run_backup_root).map_err(|err| {
        CommandError::blocked(
            "backup_root_unavailable",
            format!("failed to create backup directory: {}", run_backup_root.display()),
            "Set `PDF_HOME` or `TOOLS_HOME` to a writable directory, or rerun with `--no-backup`.",
        )
        .with_details(json!({
            "path": run_backup_root.display().to_string(),
            "source": err.to_string(),
        }))
    })?;

    pool.install(|| {
        paths.par_iter().try_for_each(|source| -> CommandResult<()> {
            let relative = source.strip_prefix("/").unwrap_or(source);
            let destination = run_backup_root.join(relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    CommandError::blocked(
                        "backup_root_unavailable",
                        format!("failed to create backup parent path: {}", parent.display()),
                        "Set `PDF_HOME` or `TOOLS_HOME` to a writable directory, or rerun with `--no-backup`.",
                    )
                    .with_details(json!({
                        "path": parent.display().to_string(),
                        "source": err.to_string(),
                    }))
                })?;
            }
            fs::copy(source, &destination).map_err(|err| {
                CommandError::blocked(
                    "backup_copy_failed",
                    format!(
                        "failed to copy backup file: {} -> {}",
                        source.display(),
                        destination.display()
                    ),
                    "Verify read and write permissions, or rerun with `--no-backup` if backups are intentionally disabled.",
                )
                .with_details(json!({
                    "sourcePath": source.display().to_string(),
                    "destinationPath": destination.display().to_string(),
                    "source": err.to_string(),
                }))
            })?;
            Ok(())
        })
    })?;

    Ok(run_backup_root)
}

fn write_report(report: &RunReport, timestamp: &str, mode: &str) -> CommandResult<PathBuf> {
    let root = reports_root()?;
    fs::create_dir_all(&root).map_err(|err| {
        CommandError::blocked(
            "report_root_unavailable",
            format!("failed to create reports root: {}", root.display()),
            "Set `PDF_HOME` or `TOOLS_HOME` to a writable directory before retrying.",
        )
        .with_details(json!({
            "path": root.display().to_string(),
            "source": err.to_string(),
        }))
    })?;
    let report_path = root.join(format!("{timestamp}-{mode}.json"));

    let payload = serde_json::to_string_pretty(report).map_err(|err| {
        CommandError::failure(
            "report_serialization_failed",
            format!("failed to serialize report: {err}"),
            "Retry the command after removing unsupported data from the report pipeline.",
        )
        .with_details(json!({
            "reportPath": report_path.display().to_string(),
            "source": err.to_string(),
        }))
    })?;
    fs::write(&report_path, payload).map_err(|err| {
        CommandError::blocked(
            "report_write_failed",
            format!("failed to write report: {}", report_path.display()),
            "Set `PDF_HOME` or `TOOLS_HOME` to a writable directory before retrying.",
        )
        .with_details(json!({
            "reportPath": report_path.display().to_string(),
            "source": err.to_string(),
        }))
    })?;
    Ok(report_path)
}
