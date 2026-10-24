import { execSync } from "node:child_process";
import { rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const crateDir = join(root, "projects/hermes.rs/crates/hermes-wasm");
const outDir = join(root, "projects/hermes.ts/platforms/hermes-unknown-wasm32/src/lib");

execSync(`wasm-pack build "${crateDir}" --target web --out-dir "${outDir}" --release`, {
    cwd: root,
    stdio: "inherit",
    shell: true,
});

// wasm-pack writes a blanket `*` ignore file into the output directory. The
// platform package owns its own `.gitignore` instead so generated `.d.ts`
// declarations stay tracked while glue and `.wasm` binaries stay local.
rmSync(join(outDir, ".gitignore"), { force: true });
rmSync(join(outDir, "package.json"), { force: true });

