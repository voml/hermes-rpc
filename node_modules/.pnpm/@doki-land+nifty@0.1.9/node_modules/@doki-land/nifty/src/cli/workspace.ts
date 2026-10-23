import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { detectProjectLayout } from "../config/detectProject.js";

export type WorkspacePackage = {
    name: string;
    version: string;
    dir: string;
    manifestPath: string;
    private: boolean;
    manifest: PackageManifest;
};

export type PackageManifest = {
    name: string;
    version: string;
    private?: boolean;
    dependencies?: Record<string, string>;
    devDependencies?: Record<string, string>;
    optionalDependencies?: Record<string, string>;
    peerDependencies?: Record<string, string>;
};

const DEPENDENCY_FIELDS = [
    "dependencies",
    "devDependencies",
    "optionalDependencies",
    "peerDependencies",
] as const;

export function findWorkspaceRoot(cwd: string): string {
    const layout = detectProjectLayout(cwd);
    const candidates = [layout.root, resolve(cwd)];
    for (const candidate of candidates) {
        const crates = join(candidate, "projects", "crates");
        const packages = join(candidate, "projects", "packages");
        if (existsSync(crates) && existsSync(packages)) {
            return candidate;
        }
    }
    throw new Error("could not find Nifty workspace (expected projects/crates and projects/packages)");
}

export function listWorkspacePackages(root: string): WorkspacePackage[] {
    const packagesDir = join(root, "projects", "packages");
    if (!existsSync(packagesDir)) {
        return [];
    }

    const packages: WorkspacePackage[] = [];
    for (const entry of readdirSync(packagesDir, { withFileTypes: true })) {
        if (!entry.isDirectory() || entry.name === "node_modules") {
            continue;
        }
        const manifestPath = join(packagesDir, entry.name, "package.json");
        if (!existsSync(manifestPath)) {
            continue;
        }
        const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as PackageManifest;
        if (!manifest.name || !manifest.version) {
            throw new Error(`package.json missing name or version: ${manifestPath}`);
        }
        packages.push({
            name: manifest.name,
            version: manifest.version,
            dir: join(packagesDir, entry.name),
            manifestPath,
            private: manifest.private === true,
            manifest,
        });
    }

    return packages.sort((a, b) => a.name.localeCompare(b.name));
}

export function collectInternalDependencyNames(
    pkg: WorkspacePackage,
    byName: Map<string, WorkspacePackage>,
): Set<string> {
    const deps = new Set<string>();
    for (const field of DEPENDENCY_FIELDS) {
        const entries = pkg.manifest[field];
        if (!entries) {
            continue;
        }
        for (const [depName, spec] of Object.entries(entries)) {
            if (byName.has(depName)) {
                deps.add(depName);
                continue;
            }
            const resolved = resolveWorkspaceDependencyName(spec, pkg.dir, byName);
            if (resolved) {
                deps.add(resolved);
            }
        }
    }
    return deps;
}

function resolveWorkspaceDependencyName(
    spec: string,
    packageDir: string,
    byName: Map<string, WorkspacePackage>,
): string | undefined {
    if (spec.startsWith("workspace:")) {
        return undefined;
    }
    if (!spec.startsWith("file:")) {
        return undefined;
    }
    const targetDir = resolve(packageDir, spec.slice("file:".length));
    for (const candidate of byName.values()) {
        if (candidate.dir === targetDir) {
            return candidate.name;
        }
    }
    const manifestPath = join(targetDir, "package.json");
    if (!existsSync(manifestPath)) {
        return undefined;
    }
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as PackageManifest;
    return manifest.name;
}
