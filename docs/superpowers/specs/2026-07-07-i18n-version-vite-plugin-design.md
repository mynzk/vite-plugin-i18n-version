# Design: `vite-plugin-i18n-version`

**Date**: 2026-07-07
**Status**: Draft (awaiting user approval)

## Problem

A Vite project that ships JSON-based i18n locales needs a stable, content-derived
version string that changes **only** when the translation payloads actually change.
The version is injected at build time so that downstream caches (CDN, browser
cache, server-side rendering, asset filenames) can be invalidated precisely.

The hot path — scanning hundreds of locale files and hashing their bytes —
belongs in Rust for speed and reproducibility.

## Goals

- Build a Vite plugin whose core logic lives in a Rust crate (`napi-rs`).
- Compute a deterministic version hash from the bytes of matched i18n files.
- Expose the hash via `config.define` so application code reads it as a
  global constant. The default key is `__I18N_VERSION__`; users who prefer
  Vite's `import.meta.env.VITE_*` convention opt into
  `defineKey: "VITE_I18N_VERSION"`.
- Ship a TypeScript-first public API with full `.d.ts` types.
- Compute the hash **once** at dev/build startup; no watch-mode re-hashing.

## Non-Goals

- Supporting non-JSON locale formats (YAML / `.po` / `.ts`).
- Watching i18n files for hot reload (intentional simplification).
- Generating a semantic version (`v1.2.3`) — the output is an opaque hash.
- Producing locale-aware runtime APIs; this plugin is build-time only.

## Constraints

- Workspace uses **pnpm**.
- npm package name: `vite-plugin-i18n-version`.
- Rust crate name: `i18n_version_core`.
- Node.js ≥ 18 (Vite 5/6 baseline).
- Pre-built `.node` binaries are published via `@napi-rs/cli` GitHub Action;
  end users do not need a Rust toolchain.

## Architecture

Two-package pnpm workspace:

```
my-rust/
├── package.json                     # pnpm workspace root
├── pnpm-workspace.yaml
├── packages/
│   └── vite-plugin-i18n-version/    # thin TS wrapper (npm package)
│       ├── package.json
│       ├── tsconfig.json
│       ├── src/
│       │   ├── index.ts             # default-exported factory
│       │   ├── types.ts             # PluginOptions
│       │   └── native.ts            # .node loader (ESM/CJS-safe)
│       ├── __tests__/
│       │   ├── fixtures/            # en.json, zh-CN.json, ja.json
│       │   └── plugin.test.ts       # vitest
│       └── index.d.ts               # public types
└── crates/
    └── i18n_version_core/           # Rust crate (napi-rs)
        ├── Cargo.toml
        ├── package.json             # napi-rs build manifest
        ├── src/
        │   ├── lib.rs               # #[napi] entry
        │   ├── scanner.rs           # directory walk
        │   ├── hash.rs              # SHA256 → truncated hex
        │   └── error.rs             # I18nVersionError
        └── __tests__/
            └── integration.rs       # napi round-trip
```

### Why two packages

- The Rust crate is independently testable with `cargo test`.
- The TS wrapper is ~50 lines and exists only to bridge Vite's plugin contract
  to the napi surface.
- Users install only the npm package; pre-built `.node` binaries are
  downloaded at install time via `@napi-rs/cli`.

## Components

### Rust crate: `i18n_version_core`

| Module | Responsibility | Key exports |
|---|---|---|
| `lib.rs` | napi boundary | `#[napi] fn compute_version(opts: ComputeOptions) -> Result<String>` |
| `scanner.rs` | Recursively walk `root` matching `include` globs; return entries sorted by relative path | `scan(root: &Path, patterns: &[String]) -> Result<Vec<FileEntry>>` |
| `hash.rs` | Concatenate `<relative_path>\0<bytes>` for each entry, SHA256, truncate to `length` hex chars | `compute_hash(entries: &[FileEntry], length: usize) -> String` |
| `error.rs` | Unified error type, mapped to `napi::Error::from_reason` | `I18nVersionError` |

### TS package: `vite-plugin-i18n-version`

| File | Responsibility |
|---|---|
| `src/index.ts` | Factory `i18nVersionPlugin(opts): Plugin` returning the Vite plugin object |
| `src/types.ts` | `PluginOptions { include: string[]; length?: number; defineKey?: string; root?: string }` |
| `src/native.ts` | Lazy ESM/CJS loader for `i18n-version-core-*.node` |

## Data Flow

```
configResolved hook
   ↓ resolve root / include / length / defineKey
   ↓ validate JSON.parse on every matched file (fail-fast)
   ↓ call compute_version({root, include, length})
[Rust] scanner::scan → sorted Vec<FileEntry>
[Rust] hash::compute_hash → SHA256 → truncated hex
   ↓
config hook
   ↓ config.define[defineKey] = JSON.stringify(hex)
   ↓
application: import.meta.env.VITE_I18N_VERSION
```

### Key design choices

- **Lexicographic sort** of file entries before hashing → identical hash
  across filesystems that return different directory iteration orders.
- **`include` uses globs**, mirroring Vite's own `assetsInclude` mental model.
- **`length` defaults to 8** (~1 in 4 × 10⁹ collision risk), overridable.
  Values are clamped to `[1, 64]` (full SHA256 hex is 64 chars); values
  outside that range are rejected with `InvalidLength`.
- **JSON validation lives in TS**, not Rust: hashing only needs stable bytes,
  parse errors should surface clearly to the user, and skipping `serde_json`
  keeps the Rust side lighter.

## Public API

```ts
// packages/vite-plugin-i18n-version/src/types.ts
export interface PluginOptions {
  /** Glob patterns relative to `root`, e.g. ["locales/**/*.json"] */
  include: string[];
  /** Project root; defaults to `process.cwd()` */
  root?: string;
  /** Hash length in hex chars; defaults to 8 */
  length?: number;
  /** Global constant key; defaults to "__I18N_VERSION__" */
  defineKey?: string;
}

export default function i18nVersionPlugin(opts: PluginOptions): import('vite').Plugin;
```

Default `defineKey` is `"__I18N_VERSION__"`. The convention matches the
familiar `import.meta.env.VITE_*` prefix when users opt into
`defineKey: "VITE_I18N_VERSION"`.

## Error Handling

| Layer | Error | User sees | Recovery |
|---|---|---|---|
| Config | `root` missing, `include` empty | `Error: i18n-version plugin: root "..." does not exist` | Fix config |
| JSON | Invalid locale file | `Error: i18n-version plugin: invalid JSON in <path>:<line>:<col>` | Fix file |
| Scan | Permission denied, etc. | `Error: i18n-version plugin: failed to scan <path>: <os error>` | Fix permissions |
| napi load | Binary / Node mismatch | `Error: i18n-version plugin: failed to load native module ...` | Reinstall |
| Soft | Single file read failure | `this.warn("skipped <path>: <reason>")` | Continue |

### Rust error type

```rust
#[derive(Debug, thiserror::Error)]
pub enum I18nVersionError {
    #[error("root path does not exist: {0}")]
    RootNotFound(String),

    #[error("failed to scan directory {path}: {source}")]
    ScanError { path: String, #[source] source: walkdir::Error },

    #[error("failed to read file {path}: {source}")]
    ReadError { path: String, #[source] source: std::io::Error },

    #[error("invalid glob pattern {pattern}: {source}")]
    InvalidGlob { pattern: String, #[source] source: glob::PatternError },

    #[error("length must be in [1, 64], got {0}")]
    InvalidLength(usize),
}
```

Mapped at the napi boundary via `napi::Error::from_reason(...)` so messages
reach the JS side intact.

### Principles

- **Fail fast**: configuration problems surface at startup, never mid-watch.
- **Actionable messages**: every error includes "what to fix".
- **No silent skips**: even soft-fail warnings name the offending file.

## Testing

### Pyramid

| Level | Where | What |
|---|---|---|
| Rust unit | `#[cfg(test)] mod tests` per module | scanner sort, hash stability, error Display |
| Rust integration | `crates/.../__tests__/integration.rs` | napi round-trip, cross-OS ordering |
| TS integration | `packages/.../__tests__/plugin.test.ts` (vitest) | hook behavior, JSON validation, native loading |

### Heart-of-the-system invariant

```rust
#[test]
fn ordering_is_stable_across_filesystems() {
    // Same files, different write order → same hash
    let dir_a = make_dir(&[("a.json","{}"), ("b.json","{}"), ("c.json","{}")]);
    let opts_a = ComputeOptions { root: dir_a, include: vec!["**/*.json".into()], length: 8 };
    let h1 = compute_version(opts_a).unwrap();

    let dir_b = make_dir(&[("c.json","{}"), ("a.json","{}"), ("b.json","{}")]);
    let opts_b = ComputeOptions { root: dir_b, include: vec!["**/*.json".into()], length: 8 };
    let h2 = compute_version(opts_b).unwrap();

    assert_eq!(h1, h2);
}
```

### Not tested (YAGNI)

- OS-specific binary compatibility — covered by CI matrix.
- Vite's internal hook ordering — Vite's responsibility.
- Edge cases of the `glob` crate — covered by its own tests.

## Dependencies

### Rust

- `napi` + `napi-derive`
- `walkdir`
- `glob`
- `sha2`
- `thiserror`

### TS (runtime)

- `vite` (peer)

### TS (dev)

- `vitest`
- `@napi-rs/cli`
- `typescript`

## Open Questions

None at design time. All decisions confirmed with the user on 2026-07-07.