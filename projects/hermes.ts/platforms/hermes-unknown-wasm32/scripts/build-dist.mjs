#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = join(pkgRoot, "..", "..", "..", "..");
const transpile = join(repoRoot, "scripts", "transpile-src-to-dist.mjs");

const run = spawnSync(process.execPath, [transpile, pkgRoot], {
    cwd: repoRoot,
    stdio: "inherit",
});
if (run.status !== 0) {
    process.exit(run.status ?? 1);
}

// The wrapper resolves `./lib/...` relative to its own module. Copy the
// wasm-pack runtime assets so `dist/index.js` finds the same paths.
const srcLib = join(pkgRoot, "src", "lib");
const distLib = join(pkgRoot, "dist", "lib");
const assets = ["hermes_wasm.js", "hermes_wasm_bg.wasm"];
mkdirSync(distLib, { recursive: true });
for (const asset of assets) {
    const from = join(srcLib, asset);
    if (!existsSync(from)) {
        console.error(`build-dist: missing ${from}; run \`node scripts/build-wasm.mjs\` first`);
        process.exit(1);
    }
    copyFileSync(from, join(distLib, asset));
}
