use sha2::{Digest, Sha256};

use crate::error::I18nVersionError;
use crate::scanner::FileEntry;

/// Incremental hasher over file entries.
///
/// Callers feed one file at a time via [`VersionHasher::update`], which lets the
/// production path stream file contents through the hash without ever holding
/// more than one file's bytes in memory at once. The byte layout per entry is
/// `<relative_path>` + `0x00` + `<contents>`, identical to the previous
/// buffer-everything implementation, so the resulting hash is unchanged.
pub struct VersionHasher {
    inner: Sha256,
}

impl VersionHasher {
    pub fn new() -> Self {
        Self { inner: Sha256::new() }
    }

    /// Feed one entry. Order matters — callers must feed entries in a stable
    /// (sorted) order to get a deterministic hash.
    pub fn update(&mut self, relative_path: &str, bytes: &[u8]) {
        self.inner.update(relative_path.as_bytes());
        self.inner.update([0u8]);
        self.inner.update(bytes);
    }

    pub fn finalize(self, length: usize) -> String {
        let digest = self.inner.finalize();
        let hex = format!("{:x}", digest);
        hex[..length].to_string()
    }
}

impl Default for VersionHasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash a fully-materialized slice of entries. Kept as a thin convenience over
/// [`VersionHasher`] for in-memory callers and tests; the streaming production
/// path in `lib.rs` drives `VersionHasher` directly.
#[allow(dead_code)]
pub fn compute_hash(entries: &[FileEntry], length: usize) -> String {
    if !(1..=64).contains(&length) {
        return "".into(); // caller should validate via InvalidLength before calling.
    }
    let mut hasher = VersionHasher::new();
    for entry in entries {
        hasher.update(&entry.relative_path, &entry.bytes);
    }
    hasher.finalize(length)
}

#[allow(dead_code)]
pub fn compute_hash_checked(
    entries: &[FileEntry],
    length: usize,
) -> Result<String, I18nVersionError> {
    if !(1..=64).contains(&length) {
        return Err(I18nVersionError::InvalidLength(length));
    }
    Ok(compute_hash(entries, length))
}
