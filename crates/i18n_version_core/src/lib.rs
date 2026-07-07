mod error;
mod hash;
mod scanner;

use std::path::PathBuf;

use napi_derive::napi;

pub use error::I18nVersionError;
use scanner::scan;

/// Options accepted from JS. All fields mirror `PluginOptions` on the TS side.
#[napi(object)]
#[derive(Clone)]
pub struct ComputeOptions {
    pub root: String,
    pub include: Vec<String>,
    pub length: u32,
}

/// Compute a deterministic hex hash of matched i18n files.
///
/// Returns the truncated hex string (length chars).
#[napi]
pub fn compute_version(opts: ComputeOptions) -> napi::Result<String> {
    let length = opts.length as usize;
    if !(1..=64).contains(&length) {
        return Err(napi::Error::from_reason(format!(
            "length must be in [1, 64], got {}",
            opts.length
        )));
    }

    let root = PathBuf::from(&opts.root);
    let entries = scan(&root, &opts.include).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(hash::compute_hash(&entries, length))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::error::I18nVersionError;

    #[test]
    fn error_display_includes_context() {
        let err = I18nVersionError::RootNotFound("/tmp/missing".into());
        assert_eq!(err.to_string(), "root path does not exist: /tmp/missing");
    }

    #[test]
    fn error_invalid_length_message() {
        let err = I18nVersionError::InvalidLength(128);
        assert_eq!(err.to_string(), "length must be in [1, 64], got 128");
    }

    #[test]
    fn scanner_returns_sorted_entries() {
        use crate::scanner::scan;
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("c.json"), "{}").unwrap();
        fs::write(dir.path().join("a.json"), "{}").unwrap();
        fs::write(dir.path().join("b.json"), "{}").unwrap();

        let entries = scan(dir.path(), &["*.json".to_string()]).unwrap();
        let names: Vec<_> = entries.iter().map(|e| e.relative_path.clone()).collect();
        assert_eq!(names, vec!["a.json", "b.json", "c.json"]);
    }

    #[test]
    fn scanner_skips_non_matching_files() {
        use crate::scanner::scan;
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.json"), "{}").unwrap();
        fs::write(dir.path().join("README.md"), "hello").unwrap();

        let entries = scan(dir.path(), &["*.json".to_string()]).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].relative_path, "a.json");
    }

    #[test]
    fn scanner_errors_on_missing_root() {
        use crate::scanner::scan;
        let result = scan(std::path::Path::new("/nonexistent-xyz"), &["*.json".into()]);
        assert!(matches!(result, Err(crate::error::I18nVersionError::RootNotFound(_))));
    }

    #[test]
    fn hash_is_deterministic_and_length_clamped() {
        use crate::hash::compute_hash;
        use crate::scanner::FileEntry;

        let entries = vec![
            FileEntry {
                relative_path: "a.json".into(),
                bytes: b"{}".to_vec(),
            },
            FileEntry {
                relative_path: "b.json".into(),
                bytes: b"{}".to_vec(),
            },
        ];

        let h8 = compute_hash(&entries, 8);
        assert_eq!(h8.len(), 8);

        let h16 = compute_hash(&entries, 16);
        assert_eq!(h16.len(), 16);
        assert!(h16.starts_with(&h8));

        let h_full = compute_hash(&entries, 64);
        assert_eq!(h_full.len(), 64);

        // Same input → same output
        assert_eq!(compute_hash(&entries, 8), h8);
    }

    #[test]
    fn hash_changes_when_entry_order_changes_after_sort() {
        use crate::hash::compute_hash;
        use crate::scanner::FileEntry;

        let e1 = FileEntry { relative_path: "a.json".into(), bytes: b"{\"x\":1}".to_vec() };
        let e2 = FileEntry { relative_path: "b.json".into(), bytes: b"{\"x\":2}".to_vec() };

        let h1 = compute_hash(&[e1.clone(), e2.clone()], 8);
        let h2 = compute_hash(&[e2, e1], 8);
        // hash module intentionally hashes in given order; scanner guarantees sort.
        // Different paths produce different hashes.
        assert_ne!(h1, h2);
    }

    #[test]
    fn compute_version_handles_cross_order_files() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("c.json"), "{}").unwrap();
        fs::write(dir.path().join("a.json"), "{}").unwrap();
        fs::write(dir.path().join("b.json"), "{}").unwrap();

        let opts = ComputeOptions {
            root: dir.path().to_string_lossy().to_string(),
            include: vec!["*.json".into()],
            length: 8,
        };

        let h1 = compute_version(opts.clone()).unwrap();
        let h2 = compute_version(opts).unwrap();
        assert_eq!(h1.len(), 8);
        assert_eq!(h1, h2);
    }

    #[test]
    fn compute_version_rejects_invalid_length() {
        let opts = ComputeOptions {
            root: ".".into(),
            include: vec!["*.json".into()],
            length: 0,
        };
        assert!(compute_version(opts).is_err());
    }
}