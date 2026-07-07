use sha2::{Digest, Sha256};

use crate::error::I18nVersionError;
use crate::scanner::FileEntry;

pub fn compute_hash(entries: &[FileEntry], length: usize) -> String {
    if !(1..=64).contains(&length) {
        return "".into(); // caller should validate via InvalidLength before calling.
    }
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update(entry.relative_path.as_bytes());
        hasher.update([0u8]);
        hasher.update(&entry.bytes);
    }
    let digest = hasher.finalize();
    let hex = format!("{:x}", digest);
    hex[..length].to_string()
}

pub fn compute_hash_checked(
    entries: &[FileEntry],
    length: usize,
) -> Result<String, I18nVersionError> {
    if !(1..=64).contains(&length) {
        return Err(I18nVersionError::InvalidLength(length));
    }
    Ok(compute_hash(entries, length))
}