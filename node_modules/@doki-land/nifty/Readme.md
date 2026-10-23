# @doki-land/nifty

[![npm version](https://img.shields.io/npm/v/@doki-land/nifty.svg)](https://www.npmjs.com/package/@doki-land/nifty)

Programmatic and CLI surface for **Nifty** — gitmoji commit lint, hybrid Cargo + npm bumps, Oak formatting, workspace publish, and gix-backed git helpers. Product overview: [npm-tools readme](https://github.com/oovm/npm-tools#readme).

## Exports

| Subpath | Purpose |
| --- | --- |
| `@doki-land/nifty` | `Nifty`, `Git`, `Github`, `Gitmoji`, config loaders, shared types |
| `@doki-land/nifty/cli` | CLI module re-exports (advanced integrations) |
| `nifty` (bin) | Full command-line interface |

Platform native bindings (`@doki-land/nifty-*`) install automatically as optional dependencies on matching hosts. Do not import them directly.

## Example

**CLI** — run from a project root that contains `nifty.config.ts` (optional):

```bash
nifty check --from v0.1.0 --to HEAD
nifty bump --version 0.2.0 --dry-run
nifty format --check
```

**TypeScript API** — open a hybrid workspace and read git history:

```ts
import { createNifty } from "@doki-land/nifty";

const nifty = await createNifty({ cwd: process.cwd() });
const ok = nifty.gitmoji.validateSubject("✨ Add workspace helper");
const range = nifty.git.resolveRange(nifty.layout.root, {
  fromRef: "v0.1.0",
  toRef: "HEAD",
});
```

**Config** — typed project defaults:

```ts
import { defineConfig } from "@doki-land/nifty";

export default defineConfig({
  lint: {
    rules: [{ id: "gitmoji/subject", severity: "error", enabled: true }],
  },
});
```

## Related packages

- [@doki-land/nifty-skills](https://www.npmjs.com/package/@doki-land/nifty-skills) — Agent Skills for `nifty` workflows
- Platform sidecars (`@doki-land/nifty-win32-x64`, `@doki-land/nifty-linux-x64`, …) — prebuilt Node-API artifacts resolved by this package
