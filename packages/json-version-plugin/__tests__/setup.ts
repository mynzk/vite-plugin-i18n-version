/**
 * Vitest setup: ensure a stub native module is available for tests that need it.
 *
 * The plugin tests exercise the full configResolved flow, which calls
 * `loadNative()` to load the real `.node` binary. The real binary is only
 * produced by the napi-rs release build (Task 12 in CI), so locally we
 * point `NATIVE_BINARY_PATH` at a tiny CommonJS stub.
 *
 * The stub emulates the native core faithfully enough for the plugin tests:
 * it walks `root`, matches `include` globs, and validates JSON — throwing
 * `invalid JSON in <file>` on the first malformed file (matching the Rust
 * `InvalidJson` error). For valid input it returns a deterministic
 * `'a'.repeat(length)` string, which satisfies the shape/regex assertions.
 *
 * If the user has set `NATIVE_BINARY_PATH` themselves (e.g. to test against
 * a real binary), we honor their override.
 */
import { writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

if (!process.env.NATIVE_BINARY_PATH) {
  const dir = mkdtempSync(join(tmpdir(), 'i18n-version-stub-'));
  const stubPath = join(dir, 'stub-native.cjs');
  writeFileSync(
    stubPath,
    `const { readdirSync, statSync, readFileSync } = require('node:fs');
const { join, relative, sep } = require('node:path');

function compileGlob(pattern) {
  const norm = pattern.replace(/\\\\/g, '/');
  let re = '^';
  for (let i = 0; i < norm.length; i++) {
    const c = norm[i];
    if (c === '*') {
      if (norm[i + 1] === '*') { re += '.*'; i++; if (norm[i + 1] === '/') i++; }
      else { re += '[^/]*'; }
    } else if (c === '?') { re += '[^/]'; }
    else if (/[.+^$(){}|[\\]\\\\]/.test(c)) { re += '\\\\' + c; }
    else { re += c; }
  }
  re += '$';
  const rx = new RegExp(re);
  return (rel) => rx.test(rel.replace(/\\\\/g, '/'));
}

module.exports = {
  computeVersion(opts) {
    const compiled = (opts.include || []).map(compileGlob);
    const matches = [];
    const visit = (abs) => {
      const st = statSync(abs);
      if (st.isDirectory()) {
        for (const child of readdirSync(abs)) visit(join(abs, child));
      } else if (st.isFile()) {
        const rel = relative(opts.root, abs).split(sep).join('/');
        if (compiled.some((m) => m(rel))) matches.push({ abs, rel });
      }
    };
    visit(opts.root);
    matches.sort((a, b) => a.rel.localeCompare(b.rel));
    for (const m of matches) {
      try { JSON.parse(readFileSync(m.abs, 'utf8')); }
      catch (e) { throw new Error('invalid JSON in ' + m.rel + ': ' + e.message); }
    }
    return 'a'.repeat(opts.length);
  },
};
`
  );
  process.env.NATIVE_BINARY_PATH = stubPath;
}