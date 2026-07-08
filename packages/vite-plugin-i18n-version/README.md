# vite-plugin-i18n-version

Compute a content hash of your JSON i18n files and inject it via `config.define`.

## Install

```sh
pnpm add -D vite-plugin-i18n-version
```

## Usage

```ts
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
```

```ts
// app code
// The value is injected as a JSON string via `config.define`, so use the
// identifier directly. It is *not* available through `import.meta.env`.
console.log(VITE_I18N_VERSION); // e.g. "a1b2c3d4"
```

## Options

- `include`: glob patterns relative to `root` (required).
- `root`: project root. Defaults to `process.cwd()`.
- `length`: hash length in hex chars (1–64). Defaults to `8`.
- `defineKey`: global constant name. Defaults to `__I18N_VERSION__`.
