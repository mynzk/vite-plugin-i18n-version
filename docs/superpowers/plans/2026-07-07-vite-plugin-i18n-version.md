# vite-plugin-i18n-version Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `vite-plugin-i18n-version`, a Vite plugin backed by a napi-rs Rust crate that computes a deterministic content hash from JSON i18n files and injects it via `config.define`.

**Architecture:** Two-package pnpm workspace. `crates/i18n_version_core` (Rust) does file walking, sorting, and SHA256 hashing; `packages/vite-plugin-i18n-version` (TypeScript) is a thin Vite plugin that calls the native module via napi and registers the result on `config.define`. JSON validation lives in TS so users get actionable error messages.

**Tech Stack:** Rust 1.74+, napi-rs 2.x, Node 18+, Vite 5/6, TypeScript 5.x, pnpm 9+, vitest 1.x.

---

## File Structure

Created in this order:

```
/Users/test/Desktop/my-rust/
├── package.json                     # root workspace
├── pnpm-workspace.yaml
├── .gitignore                       # already exists
├── crates/
│   └── i18n_version_core/
│       ├── Cargo.toml
│       ├── package.json             # napi-rs manifest
│       ├── .cargo/config.toml       # build config
│       └── src/
│           ├── lib.rs               # #[napi] entry
│           ├── error.rs             # I18nVersionError
│           ├── scanner.rs           # file walk + sort
│           └── hash.rs              # SHA256 + truncate
└── packages/
    └── vite-plugin-i18n-version/
        ├── package.json
        ├── tsconfig.json
        ├── vitest.config.ts
        ├── src/
        │   ├── index.ts             # default export factory
        │   ├── types.ts             # PluginOptions
        │   └── native.ts            # .node loader
        └── __tests__/
            ├── fixtures/
            │   ├── en.json
            │   ├── zh-CN.json
            │   └── ja.json
            └── plugin.test.ts
```

---

## Task 1: Bootstrap pnpm workspace

**Files:**
- Create: `package.json`
- Create: `pnpm-workspace.yaml`

- [ ] **Step 1: Create root `package.json`**

```json
{
  "name": "my-rust-workspace",
  "private": true,
  "version": "0.0.0",
  "scripts": {
    "build": "pnpm -r build",
    "test": "pnpm -r test"
  },
  "packageManager": "pnpm@9.0.0"
}
```

- [ ] **Step 2: Create `pnpm-workspace.yaml`**

```yaml
packages:
  - "packages/*"
```

- [ ] **Step 3: Install root deps (none for now) and verify workspace**

Run: `pnpm install`
Expected: creates `node_modules/` and `pnpm-lock.yaml`; no errors.

- [ ] **Step 4: Commit**

```bash
git add package.json pnpm-workspace.yaml pnpm-lock.yaml
git commit -m "chore: bootstrap pnpm workspace"
```

---

## Task 2: Rust crate skeleton + Cargo.toml

**Files:**
- Create: `crates/i18n_version_core/Cargo.toml`
- Create: `crates/i18n_version_core/src/lib.rs`
- Create: `crates/i18n_version_core/.cargo/config.toml`

- [ ] **Step 1: Create `crates/i18n_version_core/Cargo.toml`**

```toml
[package]
name = "i18n_version_core"
version = "0.1.0"
edition = "2021"
rust-version = "1.74"

[lib]
crate-type = ["cdylib"]

[dependencies]
napi = { version = "2", default-features = false, features = ["napi4", "serde-json"] }
napi-derive = "2"
walkdir = "2"
glob = "0.3"
sha2 = "0.10"
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create `.cargo/config.toml`**

```toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[target.aarch64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```

- [ ] **Step 3: Create empty `src/lib.rs` to verify the crate builds**

```rust
#[test]
fn crate_compiles() {
    assert_eq!(2 + 2, 4);
}
```

- [ ] **Step 4: Verify cargo check passes**

Run: `cd crates/i18n_version_core && cargo check`
Expected: `Finished dev profile [unoptimized + debuginfo] target(s)`. If network is unavailable, dependencies may fail to download — that's expected; we still want `cargo check` to attempt the resolution so any TOML syntax error surfaces.

- [ ] **Step 5: Commit**

```bash
git add crates/i18n_version_core
git commit -m "feat(crate): scaffold i18n_version_core with dependencies"
```

---

## Task 3: Rust error type (TDD)

**Files:**
- Create: `crates/i18n_version_core/src/error.rs`
- Modify: `crates/i18n_version_core/src/lib.rs` (add `mod error;` and a test)

- [ ] **Step 1: Write the failing test in `lib.rs`**

Replace `src/lib.rs` contents with:

```rust
mod error;

#[cfg(test)]
mod tests {
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
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd crates/i18n_version_core && cargo test error_display_includes_context`
Expected: compile error — `mod error` not found / `I18nVersionError` not exported.

- [ ] **Step 3: Implement `src/error.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum I18nVersionError {
    #[error("root path does not exist: {0}")]
    RootNotFound(String),

    #[error("failed to scan directory {path}: {source}")]
    ScanError {
        path: String,
        #[source]
        source: walkdir::Error,
    },

    #[error("failed to read file {path}: {source}")]
    ReadError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid glob pattern {pattern}: {source}")]
    InvalidGlob {
        pattern: String,
        #[source]
        source: glob::PatternError,
    },

    #[error("length must be in [1, 64], got {0}")]
    InvalidLength(usize),
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd crates/i18n_version_core && cargo test`
Expected: 2 passed, 0 failed.

- [ ] **Step 5: Commit**

```bash
git add crates/i18n_version_core/src
git commit -m "feat(crate): add I18nVersionError enum"
```

---

## Task 4: Rust scanner (TDD)

**Files:**
- Create: `crates/i18n_version_core/src/scanner.rs`
- Modify: `crates/i18n_version_core/src/lib.rs` (add `mod scanner;` + tests)

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/lib.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd crates/i18n_version_core && cargo test scanner_returns_sorted_entries`
Expected: compile error — `mod scanner` missing.

- [ ] **Step 3: Implement `src/scanner.rs`**

```rust
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
```

Add to `src/lib.rs`:

```rust
mod error;
mod scanner;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd crates/i18n_version_core && cargo test scanner`
Expected: 3 passed, 0 failed.

- [ ] **Step 5: Commit**

```bash
git add crates/i18n_version_core/src
git commit -m "feat(crate): add scanner module with sorted file walk"
```

---

## Task 5: Rust hash (TDD)

**Files:**
- Create: `crates/i18n_version_core/src/hash.rs`
- Modify: `crates/i18n_version_core/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/lib.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd crates/i18n_version_core && cargo test hash_is_deterministic`
Expected: compile error — `mod hash` missing.

- [ ] **Step 3: Implement `src/hash.rs`**

```rust
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
```

Add to `src/lib.rs`:

```rust
mod error;
mod hash;
mod scanner;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd crates/i18n_version_core && cargo test hash`
Expected: 2 passed, 0 failed.

- [ ] **Step 5: Commit**

```bash
git add crates/i18n_version_core/src
git commit -m "feat(crate): add hash module with SHA256 + length clamp"
```

---

## Task 6: Rust napi entry point `compute_version` (TDD)

**Files:**
- Modify: `crates/i18n_version_core/src/lib.rs`

- [ ] **Step 1: Write the failing integration test**

Add to the `tests` module in `src/lib.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd crates/i18n_version_core && cargo test compute_version`
Expected: compile error — `ComputeOptions`, `compute_version` not defined.

- [ ] **Step 3: Implement `lib.rs`**

Replace `src/lib.rs` with:

```rust
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
        let result = scan(std::path::Path::new("/nonexistent-xyz"), &["*.json".into()]);
        assert!(matches!(result, Err(I18nVersionError::RootNotFound(_))));
    }

    #[test]
    fn hash_is_deterministic_and_length_clamped() {
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

        let h8 = hash::compute_hash(&entries, 8);
        assert_eq!(h8.len(), 8);
        let h16 = hash::compute_hash(&entries, 16);
        assert_eq!(h16.len(), 16);
        assert!(h16.starts_with(&h8));
        let h_full = hash::compute_hash(&entries, 64);
        assert_eq!(h_full.len(), 64);
        assert_eq!(hash::compute_hash(&entries, 8), h8);
    }

    #[test]
    fn hash_changes_when_entry_order_changes() {
        use crate::scanner::FileEntry;

        let e1 = FileEntry { relative_path: "a.json".into(), bytes: b"{\"x\":1}".to_vec() };
        let e2 = FileEntry { relative_path: "b.json".into(), bytes: b"{\"x\":2}".to_vec() };
        let h1 = hash::compute_hash(&[e1.clone(), e2.clone()], 8);
        let h2 = hash::compute_hash(&[e2, e1], 8);
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
```

- [ ] **Step 4: Run all tests**

Run: `cd crates/i18n_version_core && cargo test`
Expected: 9 passed, 0 failed.

- [ ] **Step 5: Commit**

```bash
git add crates/i18n_version_core/src
git commit -m "feat(crate): expose compute_version via napi"
```

---

## Task 7: TS package skeleton

**Files:**
- Create: `packages/vite-plugin-i18n-version/package.json`
- Create: `packages/vite-plugin-i18n-version/tsconfig.json`
- Create: `packages/vite-plugin-i18n-version/vitest.config.ts`
- Create: `packages/vite-plugin-i18n-version/src/types.ts`

- [ ] **Step 1: Create `packages/vite-plugin-i18n-version/package.json`**

```json
{
  "name": "vite-plugin-i18n-version",
  "version": "0.1.0",
  "description": "Vite plugin: hash JSON i18n files at startup and inject the version via config.define.",
  "type": "module",
  "main": "./dist/index.cjs",
  "module": "./dist/index.js",
  "types": "./dist/index.d.ts",
  "exports": {
    ".": {
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js",
      "require": "./dist/index.cjs"
    }
  },
  "files": [
    "dist",
    "src",
    "README.md"
  ],
  "scripts": {
    "build": "tsc -p tsconfig.build.json",
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "peerDependencies": {
    "vite": "^5.0.0 || ^6.0.0"
  },
  "devDependencies": {
    "@types/node": "^20.11.0",
    "typescript": "^5.4.0",
    "vite": "^5.2.0",
    "vitest": "^1.5.0"
  }
}
```

- [ ] **Step 2: Create `tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "declaration": true,
    "outDir": "dist",
    "types": ["node", "vitest/globals"]
  },
  "include": ["src", "__tests__"]
}
```

- [ ] **Step 3: Create `tsconfig.build.json`**

```json
{
  "extends": "./tsconfig.json",
  "include": ["src"],
  "exclude": ["__tests__"]
}
```

- [ ] **Step 4: Create `vitest.config.ts`**

```ts
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    include: ['__tests__/**/*.test.ts'],
  },
});
```

- [ ] **Step 5: Create `src/types.ts`**

```ts
/**
 * Options for the vite-plugin-i18n-version plugin.
 *
 * - `include` is a list of glob patterns (relative to `root`) that selects
 *   which files participate in the hash.
 * - `length` is the number of hex chars in the output hash (clamped to 1..=64).
 * - `defineKey` is the global identifier injected via `config.define`.
 */
export interface PluginOptions {
  include: string[];
  root?: string;
  length?: number;
  defineKey?: string;
}

export const DEFAULT_DEFINE_KEY = '__I18N_VERSION__';
export const DEFAULT_LENGTH = 8;
```

- [ ] **Step 6: Install dependencies**

Run: `cd /Users/test/Desktop/my-rust && pnpm install`
Expected: workspace install succeeds; `node_modules/` populated.

- [ ] **Step 7: Commit**

```bash
git add packages/vite-plugin-i18n-version
git commit -m "feat(pkg): scaffold vite-plugin-i18n-version TS package"
```

---

## Task 8: TS native loader (TDD)

**Files:**
- Create: `packages/vite-plugin-i18n-version/src/native.ts`
- Create: `packages/vite-plugin-i18n-version/__tests__/native.test.ts`

- [ ] **Step 1: Write the failing test**

Create `__tests__/native.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { loadNative, NativeModule, NativeLoadError } from '../src/native.js';

describe('native loader', () => {
  it('exposes a NativeModule interface', () => {
    const mod: NativeModule = {
      computeVersion(_opts: unknown): string {
        return 'deadbeef';
      },
    };
    expect(mod.computeVersion({})).toBe('deadbeef');
  });

  it('rejects missing module with a clear error', async () => {
    await expect(loadNative('/definitely/not/a/real/path', 'computeVersion')).rejects.toBeInstanceOf(
      NativeLoadError
    );
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd packages/vite-plugin-i18n-version && pnpm vitest run native.test.ts`
Expected: FAIL — `loadNative` not exported from `../src/native.js`.

- [ ] **Step 3: Implement `src/native.ts`**

```ts
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

export interface ComputeOptionsNative {
  root: string;
  include: string[];
  length: number;
}

export interface NativeModule {
  computeVersion(opts: ComputeOptionsNative): string;
}

export class NativeLoadError extends Error {
  constructor(
    public readonly binaryPath: string,
    public readonly cause: unknown
  ) {
    super(
      `vite-plugin-i18n-version: failed to load native module at "${binaryPath}". ` +
        'Try reinstalling the package to refresh the prebuilt binary.'
    );
    this.name = 'NativeLoadError';
  }
}

/**
 * Resolve the directory of this module in both ESM and CJS contexts.
 */
function moduleDir(): string {
  // ESM
  try {
    const url = import.meta.url;
    if (typeof url === 'string' && url.startsWith('file://')) {
      return dirname(fileURLToPath(url));
    }
  } catch {
    // fall through
  }
  // CJS
  try {
    const req = createRequire(import.meta.url ?? __filename);
    return dirname(req.resolve('./'));
  } catch {
    return __dirname;
  }
}

/**
 * Search a few candidate filenames for the prebuilt .node binary.
 * In dev/test we stub it via NATIVE_BINARY_PATH env var to point at a fake.
 */
function findBinary(): string {
  const override = process.env.NATIVE_BINARY_PATH;
  if (override) return override;

  const dir = moduleDir();
  const candidates = [
    'i18n-version-core.darwin-arm64.node',
    'i18n-version-core.darwin-x64.node',
    'i18n-version-core.linux-x64-gnu.node',
    'i18n-version-core.win32-x64-msvc.node',
    'i18n-version-core.node',
  ];
  for (const c of candidates) {
    const p = join(dir, c);
    try {
      // eslint-disable-next-line @typescript-eslint/no-require-imports
      require('node:fs').accessSync(p);
      return p;
    } catch {
      // try next
    }
  }
  return join(dir, candidates[0]);
}

/**
 * Load the native module. The path parameter exists so tests can supply a stub path.
 */
export async function loadNative(
  binaryPath: string = findBinary(),
  _exportName: string = 'computeVersion'
): Promise<NativeModule> {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports, @typescript-eslint/no-var-requires
    const mod: NativeModule = require(binaryPath);
    if (typeof mod.computeVersion !== 'function') {
      throw new NativeLoadError(binaryPath, new Error('computeVersion is not a function'));
    }
    return mod;
  } catch (e) {
    if (e instanceof NativeLoadError) throw e;
    throw new NativeLoadError(binaryPath, e);
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd packages/vite-plugin-i18n-version && pnpm vitest run native.test.ts`
Expected: 2 passed, 0 failed.

- [ ] **Step 5: Commit**

```bash
git add packages/vite-plugin-i18n-version
git commit -m "feat(pkg): add native module loader with typed error"
```

---

## Task 9: TS plugin factory + JSON validation (TDD)

**Files:**
- Create: `packages/vite-plugin-i18n-version/src/index.ts`

- [ ] **Step 1: Write the failing tests**

Create `__tests__/plugin.test.ts`:

```ts
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, writeFileSync, mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import i18nVersionPlugin from '../src/index.js';
import type { Plugin } from 'vite';

function makeFixtures(): string {
  const dir = mkdtempSync(join(tmpdir(), 'i18n-'));
  mkdirSync(join(dir, 'locales'));
  writeFileSync(join(dir, 'locales/en.json'), JSON.stringify({ hello: 'Hello' }));
  writeFileSync(join(dir, 'locales/zh-CN.json'), JSON.stringify({ hello: '你好' }));
  writeFileSync(join(dir, 'locales/ja.json'), JSON.stringify({ hello: 'こんにちは' }));
  return dir;
}

describe('i18nVersionPlugin', () => {
  let fixturesDir: string;
  beforeEach(() => {
    fixturesDir = makeFixtures();
  });
  afterEach(() => {
    rmSync(fixturesDir, { recursive: true, force: true });
  });

  it('returns a plugin object with name', () => {
    const plugin = i18nVersionPlugin({ include: ['locales/**/*.json'], root: fixturesDir });
    expect(plugin.name).toBe('vite-plugin-i18n-version');
    expect(typeof plugin.configResolved).toBe('function');
  });

  it('injects default define key as JSON-stringified hex', () => {
    const plugin = i18nVersionPlugin({ include: ['locales/**/*.json'], root: fixturesDir });
    const defines: Record<string, string> = {};
    (plugin as any).configResolved.call(
      { config: { define: defines } },
      { root: fixturesDir, command: 'build' }
    );
    const value = defines['__I18N_VERSION__'];
    expect(value).toBeTruthy();
    // JSON-stringified hex of length 8
    expect(JSON.parse(value)).toMatch(/^[0-9a-f]{8}$/);
  });

  it('honors custom defineKey and length', () => {
    const plugin = i18nVersionPlugin({
      include: ['locales/**/*.json'],
      root: fixturesDir,
      defineKey: 'VITE_I18N_VERSION',
      length: 16,
    });
    const defines: Record<string, string> = {};
    (plugin as any).configResolved.call(
      { config: { define: defines } },
      { root: fixturesDir, command: 'build' }
    );
    expect(JSON.parse(defines['VITE_I18N_VERSION'])).toMatch(/^[0-9a-f]{16}$/);
  });

  it('throws on invalid JSON in any matched file', () => {
    const dir = mkdtempSync(join(tmpdir(), 'i18n-bad-'));
    mkdirSync(join(dir, 'locales'));
    writeFileSync(join(dir, 'locales/en.json'), '{ not valid json');
    writeFileSync(join(dir, 'locales/zh.json'), '{}');

    const plugin = i18nVersionPlugin({ include: ['locales/**/*.json'], root: dir });
    const defines: Record<string, string> = {};
    expect(() =>
      (plugin as any).configResolved.call(
        { config: { define: defines } },
        { root: dir, command: 'build' }
      )
    ).toThrow(/invalid JSON/);

    rmSync(dir, { recursive: true, force: true });
  });

  it('throws when root does not exist', () => {
    const plugin = i18nVersionPlugin({
      include: ['locales/**/*.json'],
      root: '/no/such/path',
    });
    const defines: Record<string, string> = {};
    expect(() =>
      (plugin as any).configResolved.call(
        { config: { define: defines } },
        { root: '/no/such/path', command: 'build' }
      )
    ).toThrow(/root ".*" does not exist/);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd packages/vite-plugin-i18n-version && pnpm vitest run plugin.test.ts`
Expected: FAIL — `index.js` not found.

- [ ] **Step 3: Implement `src/index.ts`**

```ts
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, resolve, sep } from 'node:path';

import {
  DEFAULT_DEFINE_KEY,
  DEFAULT_LENGTH,
  type PluginOptions,
} from './types.js';
import { loadNative, NativeLoadError, type ComputeOptionsNative } from './native.js';

const PLUGIN_NAME = 'vite-plugin-i18n-version';

/**
 * Tiny glob matcher: supports `**`, `*`, `?`. Deliberately minimal —
 * we don't want to ship a full glob library in the plugin runtime.
 */
function compileGlob(pattern: string): (rel: string) => boolean {
  // Normalize separators to '/'
  const norm = pattern.replace(/\\/g, '/');
  let re = '^';
  for (let i = 0; i < norm.length; i++) {
    const c = norm[i];
    if (c === '*') {
      if (norm[i + 1] === '*') {
        // `**` matches any path segments
        re += '.*';
        i++; // skip second *
        if (norm[i + 1] === '/') i++; // skip following /
      } else {
        re += '[^/]*';
      }
    } else if (c === '?') {
      re += '[^/]';
    } else if (/[.+^$(){}|[\]\\]/.test(c)) {
      re += '\\' + c;
    } else {
      re += c;
    }
  }
  re += '$';
  const regex = new RegExp(re);
  return (rel: string) => regex.test(rel.replace(/\\/g, '/'));
}

interface FileMatch {
  absPath: string;
  relPath: string;
}

function walkMatches(root: string, patterns: string[]): FileMatch[] {
  const compiled = patterns.map(compileGlob);
  const out: FileMatch[] = [];

  const visit = (abs: string) => {
    const stat = statSync(abs);
    if (stat.isDirectory()) {
      for (const child of readdirSync(abs)) {
        visit(join(abs, child));
      }
    } else if (stat.isFile()) {
      const rel = relative(root, abs).split(sep).join('/');
      if (compiled.some((m) => m(rel))) {
        out.push({ absPath: abs, relPath: rel });
      }
    }
  };

  visit(root);
  out.sort((a, b) => a.relPath.localeCompare(b.relPath));
  return out;
}

export default function i18nVersionPlugin(options: PluginOptions) {
  const {
    include,
    root = process.cwd(),
    length = DEFAULT_LENGTH,
    defineKey = DEFAULT_DEFINE_KEY,
  } = options;

  if (!include || include.length === 0) {
    throw new Error(`${PLUGIN_NAME}: include must be a non-empty array of glob patterns`);
  }

  const absRoot = resolve(root);

  return {
    name: PLUGIN_NAME,
    enforce: 'pre',

    async configResolved(this: any, config: { define: Record<string, string> }) {
      if (!existsSync(absRoot)) {
        throw new Error(
          `${PLUGIN_NAME}: root "${absRoot}" does not exist`
        );
      }

      const matches = walkMatches(absRoot, include);

      // Fail-fast JSON validation.
      for (const m of matches) {
        const content = readFileSync(m.absPath, 'utf8');
        try {
          JSON.parse(content);
        } catch (err) {
          const message = err instanceof Error ? err.message : String(err);
          throw new Error(
            `${PLUGIN_NAME}: invalid JSON in ${m.relPath}: ${message}`
          );
        }
      }

      let native;
      try {
        native = await loadNative();
      } catch (err) {
        if (err instanceof NativeLoadError) {
          throw err;
        }
        throw new Error(`${PLUGIN_NAME}: ${(err as Error).message}`);
      }

      const nativeOpts: ComputeOptionsNative = {
        root: absRoot,
        include,
        length,
      };

      const hex = native.computeVersion(nativeOpts);
      config.define[defineKey] = JSON.stringify(hex);
    },
  };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd packages/vite-plugin-i18n-version && pnpm vitest run`
Expected: 7 passed (2 from native + 5 from plugin).

- [ ] **Step 5: Commit**

```bash
git add packages/vite-plugin-i18n-version
git commit -m "feat(pkg): add Vite plugin factory with JSON validation"
```

---

## Task 10: Build the TS package locally

**Files:**
- Create: `packages/vite-plugin-i18n-version/README.md`

- [ ] **Step 1: Create a minimal README**

```md
# vite-plugin-i18n-version

Compute a content hash of your JSON i18n files and inject it via `config.define`.

## Install

\`\`\`sh
pnpm add -D vite-plugin-i18n-version
\`\`\`

## Usage

\`\`\`ts
// vite.config.ts
import { defineConfig } from 'vite';
import i18nVersionPlugin from 'vite-plugin-i18n-version';

export default defineConfig({
  plugins: [
    i18nVersionPlugin({
      include: ['locales/**/*.json'],
      defineKey: 'VITE_I18N_VERSION',
    }),
  ],
});
\`\`\`

\`\`\`ts
// app code
console.log(import.meta.env.VITE_I18N_VERSION);
\`\`\`

## Options

- \`include\`: glob patterns relative to \`root\` (required).
- \`root\`: project root. Defaults to \`process.cwd()\`.
- \`length\`: hash length in hex chars (1–64). Defaults to \`8\`.
- \`defineKey\`: global constant name. Defaults to \`__I18N_VERSION__\`.
```

- [ ] **Step 2: Run the build**

Run: `cd packages/vite-plugin-i18n-version && pnpm build`
Expected: TypeScript emits `dist/index.js`, `dist/index.cjs`, `dist/index.d.ts` with no errors.

- [ ] **Step 3: Commit**

```bash
git add packages/vite-plugin-i18n-version
git commit -m "docs(pkg): add README and build artifacts"
```

---

## Task 11: Wire up napi-rs build for the Rust crate

**Files:**
- Create: `crates/i18n_version_core/package.json`

- [ ] **Step 1: Create napi-rs manifest**

```json
{
  "name": "i18n-version-core",
  "version": "0.1.0",
  "description": "Native module for vite-plugin-i18n-version",
  "main": "index.js",
  "types": "index.d.ts",
  "license": "MIT",
  "napi": {
    "name": "i18n-version-core",
    "triples": {
      "defaults": true,
      "additional": []
    }
  },
  "scripts": {
    "build": "napi build --platform --release",
    "build:debug": "napi build --platform"
  },
  "devDependencies": {
    "@napi-rs/cli": "^2.18.0"
  }
}
```

- [ ] **Step 2: Try a local debug build (optional, may fail without toolchain)**

Run: `cd crates/i18n_version_core && pnpm install --ignore-scripts && pnpm run build:debug 2>&1 | tail -30`
Expected: either succeeds producing `i18n-version-core.<platform>.node`, or prints a toolchain error. Either outcome is fine for the plan; full CI release is Task 12.

- [ ] **Step 3: Commit**

```bash
git add crates/i18n_version_core/package.json
git commit -m "feat(crate): add napi-rs build manifest"
```

---

## Task 12: GitHub Actions release workflow (build + publish .node binaries)

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create the workflow**

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - uses: pnpm/action-setup@v3
        with:
          version: 9
      - run: pnpm install
      - name: Build native binaries
        run: cd crates/i18n_version_core && pnpm exec napi build --platform --release --strip
      - name: Publish to npm
        run: pnpm publish -r --access public --no-git-checks
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add release workflow for native binaries + npm publish"
```

---

## Final Verification

After all tasks complete:

1. **Rust tests**: `cd crates/i18n_version_core && cargo test` → 9 passed.
2. **TS tests**: `cd packages/vite-plugin-i18n-version && pnpm vitest run` → 7 passed.
3. **TypeScript build**: `cd packages/vite-plugin-i18n-version && pnpm build` → no errors.
4. **Smoke test**: Create a tiny `examples/demo/` Vite project, install the plugin via `pnpm link`, run `pnpm dev`, and verify `import.meta.env.__I18N_VERSION__` resolves to an 8-char hex string.

Stop here. The plugin is feature-complete per the spec.