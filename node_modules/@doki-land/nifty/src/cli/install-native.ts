import { copyFileSync, existsSync, mkdirSync, readdirSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";

import { detectProjectLayout } from "../config/detectProject.js";

/** Basename of a built .node file → platform package directory under projects/packages/. */
const PACKAGE_BY_ASSET: Record<string, string> = {
    "nifty-win32-x64-msvc.node": "nifty-win32-x64",
    "nifty-linux-x64-gnu.node": "nifty-linux-x64",
    "nifty-linux-arm64-gnu.node": "nifty-linux-arm64",
    "nifty-darwin-x64.node": "nifty-darwin-x64",
    "nifty-darwin-arm64.node": "nifty-darwin-arm64",
};

/** Matrix platforms that must be present after install (matches publish-npm.yml). */
const REQUIRED_PACKAGES = ["nifty-win32-x64", "nifty-linux-x64", "nifty-darwin-arm64"];

export type InstallNativeOptions = {
    /** Downloaded CI artifact tree (walked recursively for .node files). */
    from: string;
    cwd?: string;
};

function walkNodes(dir: string): string[] {
    const out: string[] = [];
    if (!existsSync(dir)) {
        return out;
    }
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
        const full = join(dir, entry.name);
        if (entry.isDirectory()) {
            out.push(...walkNodes(full));
        } else if (entry.isFile() && entry.name.endsWith(".node")) {
            out.push(full);
        }
    }
    return out;
}

function libHasNode(libDir: string): boolean {
    return existsSync(libDir) && readdirSync(libDir).some((name) => name.endsWith(".node"));
}

/** Copy matrix .node artifacts into platform package lib dirs for npm publish. */
export function installNativeFromOptions(options: InstallNativeOptions): void {
    const layout = detectProjectLayout(options.cwd ?? process.cwd());
    const artifactRoot = resolve(layout.root, options.from);
    const nodes = walkNodes(artifactRoot);
    if (nodes.length === 0) {
        throw new Error(`install-native: no .node files under ${artifactRoot}`);
    }

    let installed = 0;
    for (const file of nodes) {
        const base = basename(file);
        const packageDir = PACKAGE_BY_ASSET[base];
        if (!packageDir) {
            console.log(`skip unmapped artifact ${relative(artifactRoot, file)}`);
            continue;
        }
        const dest = join(layout.root, "projects/packages", packageDir, "lib", base);
        mkdirSync(dirname(dest), { recursive: true });
        copyFileSync(file, dest);
        console.log(`installed ${relative(layout.root, dest)}`);
        installed += 1;
    }

    if (installed === 0) {
        throw new Error("install-native: no mapped platform .node files installed");
    }

    for (const packageDir of REQUIRED_PACKAGES) {
        const libDir = join(layout.root, "projects/packages", packageDir, "lib");
        if (!libHasNode(libDir)) {
            throw new Error(`install-native: missing .node under ${packageDir}/lib`);
        }
    }

    console.log(`install-native: ${installed} native binary(ies) ready for npm publish`);
}
