use serde::Serialize;

pub const TARGET_PDF_VERSION: &str = "1.7";
pub const KEEP_DOCINFO_FIELDS: [&str; 3] = ["Title", "CreationDate", "ModDate"];

pub const ACTION_SET_TITLE: &str = "set_title";
pub const ACTION_STRIP_DOCINFO: &str = "strip_docinfo_fields";
pub const ACTION_REMOVE_XMP: &str = "remove_xmp_metadata";
pub const ACTION_SET_VERSION: &str = "set_pdf_version_1_7";
pub const ACTION_OPTIMIZE: &str = "optimize_pdf_size";

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub apply: bool,
    pub estimate_size: bool,
    pub min_size_savings_bytes: u64,
    pub min_size_savings_percent: f64,
    pub jobs: Option<usize>,
    pub no_backup: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilePlan {
    pub path: String,
    pub size_bytes: u64,
    pub skipped: bool,
    pub skip_reason: String,
    pub changed: bool,
    pub planned_actions: Vec<String>,
    pub title_before: String,
    pub title_after: String,
    pub fields_to_strip: Vec<String>,
    pub xmp_present: bool,
    pub version_before: String,
    pub version_after: String,
    pub signed: bool,
    pub password_protected: bool,
    pub optimization_checked: bool,
    pub optimization_recommended: bool,
    pub optimization_error: String,
    pub estimated_size_after_bytes: Option<u64>,
    pub estimated_saved_bytes: Option<i64>,
    pub estimated_saved_percent: Option<f64>,
    pub applied: bool,
    pub apply_error: String,
    pub apply_note: String,
    pub size_after_bytes: Option<u64>,
    pub actual_saved_bytes: Option<i64>,
    pub actual_saved_percent: Option<f64>,
    #[serde(skip_serializing)]
    pub staged_optimized_path: Option<String>,
}

impl FilePlan {
    pub fn new(path: String, size_bytes: u64) -> Self {
        Self {
            path,
            size_bytes,
            skipped: false,
            skip_reason: String::new(),
            changed: false,
            planned_actions: Vec::new(),
            title_before: String::new(),
            title_after: String::new(),
            fields_to_strip: Vec::new(),
            xmp_present: false,
            version_before: String::new(),
            version_after: TARGET_PDF_VERSION.to_string(),
            signed: false,
            password_protected: false,
            optimization_checked: false,
            optimization_recommended: false,
            optimization_error: String::new(),
            estimated_size_after_bytes: None,
            estimated_saved_bytes: None,
            estimated_saved_percent: None,
            applied: false,
            apply_error: String::new(),
            apply_note: String::new(),
            size_after_bytes: None,
            actual_saved_bytes: None,
            actual_saved_percent: None,
            staged_optimized_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RunSummary {
    pub total: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub applied: usize,
    pub failed: usize,
    pub signed_total: usize,
    pub signed_skipped: usize,
    pub password_protected_skipped: usize,
    pub optimization_checked: usize,
    pub optimization_recommended: usize,
    pub estimated_saved_total_bytes: i64,
    pub actual_saved_total_bytes: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunReport {
    pub timestamp: String,
    pub mode: String,
    pub target_path: String,
    pub options: RunOptionsView,
    pub summary: RunSummary,
    pub backup_root: String,
    pub report_path: String,
    pub files: Vec<FilePlan>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunOptionsView {
    pub apply: bool,
    pub estimate_size: bool,
    pub min_size_savings_bytes: u64,
    pub min_size_savings_percent: f64,
    pub jobs: Option<usize>,
    pub no_backup: bool,
}

impl From<&RunOptions> for RunOptionsView {
    fn from(value: &RunOptions) -> Self {
        Self {
            apply: value.apply,
            estimate_size: value.estimate_size,
            min_size_savings_bytes: value.min_size_savings_bytes,
            min_size_savings_percent: value.min_size_savings_percent,
            jobs: value.jobs,
            no_backup: value.no_backup,
        }
    }
}

pub fn summarize(plans: &[FilePlan]) -> RunSummary {
    let total = plans.len();
    let skipped = plans.iter().filter(|p| p.skipped).count();
    let changed = plans.iter().filter(|p| p.changed && !p.skipped).count();
    let unchanged = plans.iter().filter(|p| !p.changed && !p.skipped).count();
    let applied = plans.iter().filter(|p| p.applied).count();
    let failed = plans.iter().filter(|p| !p.apply_error.is_empty()).count();
    let signed_total = plans.iter().filter(|p| p.signed).count();
    let signed_skipped = plans.iter().filter(|p| p.signed && p.skipped).count();
    let password_protected_skipped = plans.iter().filter(|p| p.password_protected && p.skipped).count();
    let optimization_checked = plans.iter().filter(|p| p.optimization_checked).count();
    let optimization_recommended = plans.iter().filter(|p| p.optimization_recommended).count();
    let estimated_saved_total_bytes = plans.iter().filter_map(|p| p.estimated_saved_bytes).sum::<i64>();
    let actual_saved_total_bytes = plans.iter().filter_map(|p| p.actual_saved_bytes).sum::<i64>();

    RunSummary {
        total,
        changed,
        unchanged,
        skipped,
        applied,
        failed,
        signed_total,
        signed_skipped,
        password_protected_skipped,
        optimization_checked,
        optimization_recommended,
        estimated_saved_total_bytes,
        actual_saved_total_bytes,
    }
}

pub fn is_metadata_action(action: &str) -> bool {
    matches!(
        action,
        ACTION_SET_TITLE | ACTION_STRIP_DOCINFO | ACTION_REMOVE_XMP | ACTION_SET_VERSION
    )
}
