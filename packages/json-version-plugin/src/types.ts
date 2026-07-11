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

export const DEFAULT_DEFINE_KEY = '__JSON_VERSION__';
export const DEFAULT_LENGTH = 8;