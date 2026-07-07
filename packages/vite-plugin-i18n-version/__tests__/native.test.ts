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