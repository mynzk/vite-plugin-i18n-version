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

    async configResolved(config: any) {
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
