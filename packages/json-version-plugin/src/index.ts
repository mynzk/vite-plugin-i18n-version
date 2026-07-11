import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

import {
  DEFAULT_DEFINE_KEY,
  DEFAULT_LENGTH,
  type PluginOptions,
} from './types.js';
import { loadNative, NativeLoadError, type ComputeOptionsNative } from './native.js';

const PLUGIN_NAME = 'json-version-plugin';

export default function jsonVersionPlugin(options: PluginOptions) {
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

      // The native core walks + reads + validates JSON + hashes in a single
      // pass. JSON validation failures surface here as an error whose message
      // contains "invalid JSON in <file>".
      const hex = native.computeVersion(nativeOpts);

      config.define[defineKey] = JSON.stringify(hex);
    },
  };
}
