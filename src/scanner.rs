use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::json;
use walkdir::WalkDir;

use crate::error::{CommandError, CommandResult};

pub fn collect_pdf_paths(target: &Path) -> CommandResult<Vec<PathBuf>> {
    let metadata = fs::symlink_metadata(target).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            return CommandError::failure(
                "target_not_found",
                format!("target not found: {}", target.display()),
                "Pass an existing PDF file or directory path.",
            )
            .with_details(json!({ "path": target.display().to_string() }));
        }
        CommandError::blocked(
            "target_metadata_unavailable",
            format!("failed to inspect target: {}", target.display()),
            "Verify the target path is readable and retry.",
        )
        .with_details(json!({
            "path": target.display().to_string(),
            "source": err.to_string(),
        }))
    })?;

    if metadata.file_type().is_symlink() {
        return Err(CommandError::failure(
            "symlink_target_not_allowed",
            format!("symlink target not allowed: {}", target.display()),
            "Pass a real PDF file or directory path instead of a symlink.",
        )
        .with_details(json!({ "path": target.display().to_string() })));
    }

    if is_hidden_path(target) {
        return Err(CommandError::failure(
            "hidden_target_not_allowed",
            format!("hidden target not allowed: {}", target.display()),
            "Move the target out of hidden paths or pass a visible directory.",
        )
        .with_details(json!({ "path": target.display().to_string() })));
    }

    let mut paths = Vec::new();

    if metadata.is_file() {
        if !is_pdf(target) {
            return Err(CommandError::failure(
                "target_not_pdf",
                format!("target is not a pdf file: {}", target.display()),
                "Pass a `.pdf` file or a directory containing PDFs.",
            )
            .with_details(json!({ "path": target.display().to_string() })));
        }
        paths.push(target.to_path_buf());
        return Ok(paths);
    }

    if !metadata.is_dir() {
        return Err(CommandError::failure(
            "unsupported_target_type",
            format!("unsupported target type: {}", target.display()),
            "Pass a PDF file or a directory containing PDFs.",
        )
        .with_details(json!({ "path": target.display().to_string() })));
    }

    let iter = WalkDir::new(target)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            !is_hidden_name(entry.file_name())
        });

    for entry in iter {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        if !is_pdf(path) {
            continue;
        }
        if is_hidden_name(entry.file_name()) {
            continue;
        }
        paths.push(path.to_path_buf());
    }

    paths.sort_unstable();
    paths.dedup();
    Ok(paths)
}

pub fn is_hidden_path(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => is_hidden_name(name),
        _ => false,
    })
}

fn is_hidden_name(name: &OsStr) -> bool {
    let text = name.to_string_lossy();
    text.starts_with('.') || text.starts_with("._")
}

pub fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;

    use super::{collect_pdf_paths, is_hidden_path, is_pdf};

    #[test]
    fn detects_hidden_paths() {
        assert!(is_hidden_path(Path::new("/tmp/.private/file.pdf")));
        assert!(is_hidden_path(Path::new("/tmp/._ghost.pdf")));
        assert!(!is_hidden_path(Path::new("/tmp/visible/file.pdf")));
    }

    #[test]
    fn detects_pdf_extension_case_insensitive() {
        assert!(is_pdf(Path::new("a.pdf")));
        assert!(is_pdf(Path::new("a.PDF")));
        assert!(!is_pdf(Path::new("a.txt")));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_targets() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("tempdir");
        let real = temp.path().join("real.pdf");
        fs::write(&real, b"%PDF-1.4").expect("write real pdf");
        let link = temp.path().join("linked.pdf");
        symlink(&real, &link).expect("create symlink");

        let err = collect_pdf_paths(&link).expect_err("symlink target should be rejected");
        assert_eq!(err.code(), "symlink_target_not_allowed");
    }

    #[test]
    fn rejects_hidden_directory_targets() {
        let temp = tempdir().expect("tempdir");
        let hidden = temp.path().join(".hidden");
        fs::create_dir(&hidden).expect("create hidden directory");

        let err = collect_pdf_paths(&hidden).expect_err("hidden directory should be rejected");
        assert_eq!(err.code(), "hidden_target_not_allowed");
    }
}
