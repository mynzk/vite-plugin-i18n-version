import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

// ESM has no `require` in scope; create one anchored to this module so we can
// load the prebuilt .node binary synchronously regardless of caller context
// (Vite, Jest, tsx, etc.). The previous bare `require()` calls only worked in
// CJS — under ESM the loader threw "require is not defined" and the try/catch
// in findBinary silently swallowed the error, returning a non-existent path.
const nodeRequire = createRequire(import.meta.url);

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
      `json-version-plugin: failed to load native module at "${binaryPath}". ` +
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
    const triCandidates =
      process.platform === 'linux'
        ? [`json-version-core.linux-${process.arch}-gnu.node`,
           `json-version-core.linux-${process.arch}-musl.node`]
        : [`json-version-core.${process.platform}-${process.arch}.node`];

    for (const c of triCandidates) {
      const p = join(dir, c);
      try { nodeRequire('node:fs').accessSync(p); return p; } catch {}
    }
    // 都不存在 → 抛错并把当前 platform/arch 写进错误信息
    throw new NativeLoadError(
      join(dir, triCandidates[0]),
      new Error(`no prebuilt binary for ${process.platform}/${process.arch}`)
    );
  }

/**
 * Load the native module. The path parameter exists so tests can supply a stub path.
 */
export async function loadNative(
  binaryPath: string = findBinary(),
  _exportName: string = 'computeVersion'
): Promise<NativeModule> {
  try {
    const mod: NativeModule = nodeRequire(binaryPath);
    if (typeof mod.computeVersion !== 'function') {
      throw new NativeLoadError(binaryPath, new Error('computeVersion is not a function'));
    }
    return mod;
  } catch (e) {
    if (e instanceof NativeLoadError) throw e;
    throw new NativeLoadError(binaryPath, e);
  }
}