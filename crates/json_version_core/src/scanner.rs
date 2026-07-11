use std::fs;
use std::path::{Path, PathBuf};

use glob::Pattern;
use walkdir::{DirEntry, WalkDir};

use crate::error::I18nVersionError;

/// A file entry with its contents fully read into memory. Retained for the
/// buffered [`scan`] helper and tests; the production path streams instead.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

/// A matched file located during discovery, before its contents are read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRef {
    pub relative_path: String,
    pub abs_path: PathBuf,
}

/// Directories pruned during the walk. These never contain i18n source files
/// worth hashing, and descending into them (especially `node_modules`) is the
/// single biggest avoidable cost when `root` points at a project directory.
const PRUNED_DIRS: &[&str] = &["node_modules", ".git"];

fn is_pruned_dir(entry: &DirEntry) -> bool {
    // Only prune directories, and never the walk root itself (depth 0).
    if entry.depth() == 0 || !entry.file_type().is_dir() {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .map(|name| PRUNED_DIRS.contains(&name))
        .unwrap_or(false)
}

/// Walk `root` once, pruning heavy directories, and return the matched files
/// (paths only — no I/O on file contents) sorted by relative path.
pub fn discover(root: &Path, patterns: &[String]) -> Result<Vec<FileRef>, I18nVersionError> {
    if !root.exists() {
        return Err(I18nVersionError::RootNotFound(root.display().to_string()));
    }

    let compiled: Vec<Pattern> = patterns
        .iter()
        .map(|p| {
            Pattern::new(p).map_err(|source| I18nVersionError::InvalidGlob {
                pattern: p.clone(),
                source,
            })
        })
        .collect::<Result<_, _>>()?;

    let mut refs: Vec<FileRef> = Vec::new();

    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_pruned_dir(e));

    for entry in walker {
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

        refs.push(FileRef {
            relative_path: rel,
            abs_path: abs.to_path_buf(),
        });
    }

    refs.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(refs)
}

/// Read one matched file and fail-fast validate it as JSON. Shared by both the
/// buffered [`scan`] path and the streaming hash path so validation and read
/// semantics can never diverge between them.
pub fn read_and_validate(abs: &Path, rel: &str) -> Result<Vec<u8>, I18nVersionError> {
    let bytes = fs::read(abs).map_err(|source| I18nVersionError::ReadError {
        path: abs.display().to_string(),
        source,
    })?;

    if let Err(source) = serde_json::from_slice::<serde_json::Value>(&bytes) {
        return Err(I18nVersionError::InvalidJson {
            path: rel.to_string(),
            message: source.to_string(),
        });
    }

    Ok(bytes)
}

/// Discover + read + validate every matched file into memory, sorted by
/// relative path. Convenience for in-memory callers and tests; the production
/// path streams instead (see `lib.rs`) to keep memory at O(one file).
#[allow(dead_code)]
pub fn scan(root: &Path, patterns: &[String]) -> Result<Vec<FileEntry>, I18nVersionError> {
    let refs = discover(root, patterns)?;
    let mut entries = Vec::with_capacity(refs.len());
    for r in refs {
        let bytes = read_and_validate(&r.abs_path, &r.relative_path)?;
        entries.push(FileEntry {
            relative_path: r.relative_path,
            bytes,
        });
    }
    Ok(entries)
}

/// Convenience wrapper: walk a single root and join entries, sorted by relative path.
#[allow(dead_code)]
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
