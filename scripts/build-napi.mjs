import { execSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, unlinkSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = join(root, "projects/hermes-napi/Cargo.toml");
const napiDir = join(root, "projects/hermes-napi");

/** @type {Record<string, { packageDir: string; fileName: string }>} */
const ARTIFACTS = {
    "hermes.win32-x64-msvc.node": {
        packageDir: "hermes-win32-x64",
        fileName: "hermes-win32-x64-msvc.node",
    },
    "hermes.linux-x64-gnu.node": {
        packageDir: "hermes-linux-x64",
        fileName: "hermes-linux-x64-gnu.node",
    },
    "hermes.linux-arm64-gnu.node": {
        packageDir: "hermes-linux-arm64",
        fileName: "hermes-linux-arm64-gnu.node",
    },
    "hermes.darwin-x64.node": {
        packageDir: "hermes-darwin-x64",
        fileName: "hermes-darwin-x64.node",
    },
    "hermes.darwin-arm64.node": {
        packageDir: "hermes-darwin-arm64",
        fileName: "hermes-darwin-arm64.node",
    },
    // @napi-rs/cli v2 artifact names (fallback when upgrading incrementally).
    "index.win32-x64-msvc.node": {
        packageDir: "hermes-win32-x64",
        fileName: "hermes-win32-x64-msvc.node",
    },
    "index.linux-x64-gnu.node": {
        packageDir: "hermes-linux-x64",
        fileName: "hermes-linux-x64-gnu.node",
    },
    "index.linux-arm64-gnu.node": {
        packageDir: "hermes-linux-arm64",
        fileName: "hermes-linux-arm64-gnu.node",
    },
    "index.darwin-x64.node": {
        packageDir: "hermes-darwin-x64",
        fileName: "hermes-darwin-x64.node",
    },
    "index.darwin-arm64.node": {
        packageDir: "hermes-darwin-arm64",
        fileName: "hermes-darwin-arm64.node",
    },
};

function findBuiltNodes(dir) {
    return readdirSync(dir).filter((name) => name.endsWith(".node"));
}

execSync(
    `pnpm exec napi build --platform --release --manifest-path "${manifestPath.replace(/\\/g, "/")}"`,
    { cwd: root, stdio: "inherit", shell: true },
);

const built = findBuiltNodes(napiDir);
if (built.length === 0) {
    throw new Error(`No .node artifacts under ${napiDir}`);
}

let copied = 0;
for (const name of built) {
    const mapping = ARTIFACTS[name];
    if (!mapping) {
        console.warn(`skip unmapped artifact: ${name}`);
        continue;
    }
    const libDir = join(root, "projects/packages", mapping.packageDir, "lib");
    mkdirSync(libDir, { recursive: true });
    for (const existing of readdirSync(libDir).filter((entry) => entry.endsWith(".node"))) {
        unlinkSync(join(libDir, existing));
    }
    const dest = join(libDir, mapping.fileName);
    copyFileSync(join(napiDir, name), dest);
    console.log(`copied ${name} -> ${dest}`);
    copied += 1;
}

if (copied === 0) {
    throw new Error(`Built nodes (${built.join(", ")}) did not match any platform package mapping`);
}
