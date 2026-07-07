use std::fs;
use std::path::{Path, PathBuf};

use glob::Pattern;
use walkdir::WalkDir;

use crate::error::I18nVersionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

pub fn scan(root: &Path, patterns: &[String]) -> Result<Vec<FileEntry>, I18nVersionError> {
    if !root.exists() {
        return Err(I18nVersionError::RootNotFound(root.display().to_string()));
    }

    let compiled: Vec<Pattern> = patterns
        .iter()
        .map(|p| {
            Pattern::new(p)
                .map_err(|source| I18nVersionError::InvalidGlob {
                    pattern: p.clone(),
                    source,
                })
        })
        .collect::<Result<_, _>>()?;

    let mut entries: Vec<FileEntry> = Vec::new();

    for entry in WalkDir::new(root).follow_links(false).into_iter() {
        let entry = entry.map_err(|source| I18nVersionError::ScanError {
            path: root.display().to_string(),
            source,
        })?;

        if !entry.file_type().is_file() {
            continue;
        }

        let abs = entry.path();
        let rel = abs
            .strip_prefix(root)
            .map_err(|source| I18nVersionError::ReadError {
                path: abs.display().to_string(),
                source: std::io::Error::new(std::io::ErrorKind::Other, source.to_string()),
            })?
            .to_string_lossy()
            .replace('\\', "/");

        if !compiled.iter().any(|p| p.matches(&rel)) {
            continue;
        }

        let bytes = fs::read(abs).map_err(|source| I18nVersionError::ReadError {
            path: abs.display().to_string(),
            source,
        })?;

        entries.push(FileEntry {
            relative_path: rel,
            bytes,
        });
    }

    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(entries)
}

/// Convenience wrapper: walk a single root and join entries, sorted by relative path.
pub fn scan_many<I, P>(roots: I, patterns: &[String]) -> Result<Vec<FileEntry>, I18nVersionError>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut all = Vec::new();
    for r in roots {
        let mut from_root = scan(r.as_ref(), patterns)?;
        // Prefix entries with the root alias for uniqueness across multiple roots.
        // For single-root usage this is a no-op prefix "".
        let alias = r.as_ref().to_string_lossy().replace('\\', "/");
        for entry in &mut from_root {
            entry.relative_path = if alias.is_empty() {
                entry.relative_path.clone()
            } else {
                format!("{}/{}", alias.trim_start_matches('/'), entry.relative_path)
            };
        }
        all.extend(from_root);
    }
    all.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(all)
}

#[allow(dead_code)]
fn _ensure_send<T: Send>() {}
#[allow(dead_code)]
fn _ensure_path(_: PathBuf) {}
