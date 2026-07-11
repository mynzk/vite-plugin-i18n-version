import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, writeFileSync, mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import i18nVersionPlugin from '../src/index.js';

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

  it('injects default define key as JSON-stringified hex', async () => {
    const plugin = i18nVersionPlugin({ include: ['locales/**/*.json'], root: fixturesDir });
    const defines: Record<string, string> = {};
    await (plugin as any).configResolved(
      { root: fixturesDir, command: 'build', define: defines }
    );
    const value = defines['__I18N_VERSION__'];
    expect(value).toBeTruthy();
    expect(JSON.parse(value)).toMatch(/^[0-9a-f]{8}$/);
  });

  it('honors custom defineKey and length', async () => {
    const plugin = i18nVersionPlugin({
      include: ['locales/**/*.json'],
      root: fixturesDir,
      defineKey: 'VITE_I18N_VERSION',
      length: 16,
    });
    const defines: Record<string, string> = {};
    await (plugin as any).configResolved(
      { root: fixturesDir, command: 'build', define: defines }
    );
    expect(JSON.parse(defines['VITE_I18N_VERSION'])).toMatch(/^[0-9a-f]{16}$/);
  });

  it('throws on invalid JSON in any matched file', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'i18n-bad-'));
    mkdirSync(join(dir, 'locales'));
    writeFileSync(join(dir, 'locales/en.json'), '{ not valid json');
    writeFileSync(join(dir, 'locales/zh.json'), '{}');

    const plugin = i18nVersionPlugin({ include: ['locales/**/*.json'], root: dir });
    const defines: Record<string, string> = {};
    await expect(
      (plugin as any).configResolved(
        { root: dir, command: 'build', define: defines }
      )
    ).rejects.toThrow(/invalid JSON/);

    rmSync(dir, { recursive: true, force: true });
  });

  it('throws when root does not exist', async () => {
    const plugin = i18nVersionPlugin({
      include: ['locales/**/*.json'],
      root: '/no/such/path',
    });
    const defines: Record<string, string> = {};
    await expect(
      (plugin as any).configResolved(
        { root: '/no/such/path', command: 'build', define: defines }
      )
    ).rejects.toThrow(/root ".*" does not exist/);
  });
});
