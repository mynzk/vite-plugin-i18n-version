/**
 * Vitest setup: ensure a stub native module is available for tests that need it.
 *
 * The plugin tests exercise the full configResolved flow, which calls
 * `loadNative()` to load the real `.node` binary. The real binary is only
 * produced by the napi-rs release build (Task 12 in CI), so locally we
 * point `NATIVE_BINARY_PATH` at a tiny CommonJS stub. The stub returns a
 * deterministic `'a'.repeat(length)` string — sufficient for tests that
 * only check shape/regex, not actual hash values.
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
    `module.exports = {
  computeVersion(opts) {
    return 'a'.repeat(opts.length);
  },
};
`
  );
  process.env.NATIVE_BINARY_PATH = stubPath;
}