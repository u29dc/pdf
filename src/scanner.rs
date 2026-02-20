use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use walkdir::WalkDir;

pub fn collect_pdf_paths(target: &Path) -> Result<Vec<PathBuf>> {
    if !target.exists() {
        return Err(anyhow!("target not found: {}", target.display()));
    }

    let mut set = BTreeSet::new();

    if target.is_file() {
        if !is_pdf(target) {
            return Err(anyhow!("target is not a pdf file: {}", target.display()));
        }
        if is_hidden_path(target) {
            return Err(anyhow!("hidden target not allowed: {}", target.display()));
        }
        let resolved = target
            .canonicalize()
            .with_context(|| format!("failed to resolve target: {}", target.display()))?;
        set.insert(resolved);
        return Ok(set.into_iter().collect());
    }

    if !target.is_dir() {
        return Err(anyhow!("unsupported target type: {}", target.display()));
    }

    let root = target
        .canonicalize()
        .with_context(|| format!("failed to resolve directory: {}", target.display()))?;

    let iter = WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.path() == root {
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
        if !path.is_file() {
            continue;
        }
        if !is_pdf(path) {
            continue;
        }
        if is_hidden_path(path) {
            continue;
        }
        let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        set.insert(resolved);
    }

    Ok(set.into_iter().collect())
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
    use std::path::Path;

    use super::{is_hidden_path, is_pdf};

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
}
