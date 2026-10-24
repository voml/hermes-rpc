#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const pkgRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = join(pkgRoot, "..", "..", "..", "..");
const transpile = join(repoRoot, "scripts", "transpile-src-to-dist.mjs");

function run(command, args) {
    const result = spawnSync(command, args, { cwd: repoRoot, stdio: "inherit" });
    if (result.status !== 0) {
        process.exit(result.status ?? 1);
    }
}

run(process.execPath, [transpile, pkgRoot]);
run(process.execPath, [require.resolve("typescript/bin/tsc"), "-p", join(pkgRoot, "tsconfig.build.json")]);
