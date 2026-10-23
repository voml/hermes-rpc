import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

export type ProjectKind = "cargo" | "npm" | "hybrid" | "unknown";

/** Detected cargo + npm project layout. */
export type ProjectLayout = {
    /** Resolved project root (git root when present). */
    root: string;
    kind: ProjectKind;
    /** Nearest `Cargo.toml` walking upward. */
    cargoManifest?: string;
    /** Nearest `package.json` walking upward (skips `node_modules`). */
    packageManifest?: string;
    /** Directory whose `Cargo.toml` declares a `[workspace]`. */
    cargoWorkspaceRoot?: string;
    /** Directory whose `package.json` declares npm `workspaces`. */
    npmWorkspaceRoot?: string;
};

const CARGO_MANIFEST = "Cargo.toml";
const PACKAGE_MANIFEST = "package.json";
const GIT_DIR = ".git";

function isInsideNodeModules(path: string): boolean {
    return path.split(/[/\\]/).includes("node_modules");
}

function findFileUpward(cwd: string, fileName: string, skipNodeModules: boolean): string | undefined {
    let here = resolve(cwd);
    while (true) {
        if (skipNodeModules && isInsideNodeModules(here)) {
            const parent = dirname(here);
            if (parent === here) {
                return undefined;
            }
            here = parent;
            continue;
        }
        const candidate = join(here, fileName);
        if (existsSync(candidate)) {
            return candidate;
        }
        const parent = dirname(here);
        if (parent === here) {
            return undefined;
        }
        here = parent;
    }
}

function findGitRoot(cwd: string): string | undefined {
    let here = resolve(cwd);
    while (true) {
        if (existsSync(join(here, GIT_DIR))) {
            return here;
        }
        const parent = dirname(here);
        if (parent === here) {
            return undefined;
        }
        here = parent;
    }
}

function manifestHasCargoWorkspace(manifestPath: string): boolean {
    try {
        return readFileSync(manifestPath, "utf8")
            .split(/\r?\n/)
            .some((line) => line.trim() === "[workspace]");
    } catch {
        return false;
    }
}

function manifestHasNpmWorkspaces(manifestPath: string): boolean {
    try {
        return readFileSync(manifestPath, "utf8").includes('"workspaces"');
    } catch {
        return false;
    }
}

function findCargoWorkspaceRoot(manifestPath: string): string | undefined {
    let here = dirname(resolve(manifestPath));
    while (true) {
        const candidate = join(here, CARGO_MANIFEST);
        if (existsSync(candidate) && manifestHasCargoWorkspace(candidate)) {
            return here;
        }
        const parent = dirname(here);
        if (parent === here) {
            return undefined;
        }
        here = parent;
    }
}

function findNpmWorkspaceRoot(manifestPath: string): string | undefined {
    let here = dirname(resolve(manifestPath));
    while (true) {
        const candidate = join(here, PACKAGE_MANIFEST);
        if (existsSync(candidate) && manifestHasNpmWorkspaces(candidate)) {
            return here;
        }
        const parent = dirname(here);
        if (parent === here) {
            return undefined;
        }
        here = parent;
    }
}

function classifyProject(hasCargo: boolean, hasNpm: boolean): ProjectKind {
    if (hasCargo && hasNpm) {
        return "hybrid";
    }
    if (hasCargo) {
        return "cargo";
    }
    if (hasNpm) {
        return "npm";
    }
    return "unknown";
}

/** Walk upward and return the nearest `Cargo.toml`. */
export function findCargoManifest(cwd = process.cwd()): string | undefined {
    return findFileUpward(cwd, CARGO_MANIFEST, false);
}

/** Walk upward and return the nearest `package.json` (skips `node_modules`). */
export function findPackageManifest(cwd = process.cwd()): string | undefined {
    return findFileUpward(cwd, PACKAGE_MANIFEST, true);
}

/** Detect cargo/npm/hybrid layout from `cwd`. */
export function detectProjectLayout(cwd = process.cwd()): ProjectLayout {
    const start = resolve(cwd);
    const cargoManifest = findCargoManifest(start);
    const packageManifest = findPackageManifest(start);
    const cargoWorkspaceRoot = cargoManifest ? findCargoWorkspaceRoot(cargoManifest) : undefined;
    const npmWorkspaceRoot = packageManifest ? findNpmWorkspaceRoot(packageManifest) : undefined;
    const kind = classifyProject(Boolean(cargoManifest), Boolean(packageManifest));
    const root =
        findGitRoot(start) ??
        cargoWorkspaceRoot ??
        npmWorkspaceRoot ??
        (cargoManifest ? dirname(cargoManifest) : undefined) ??
        (packageManifest ? dirname(packageManifest) : undefined) ??
        start;

    return {
        root,
        kind,
        cargoManifest,
        packageManifest,
        cargoWorkspaceRoot,
        npmWorkspaceRoot,
    };
}

/** Discover npm package directories for hybrid monorepos. */
export function discoverPackageDirs(layout: ProjectLayout): string[] {
    const roots = new Set<string>();

    if (layout.npmWorkspaceRoot) {
        roots.add(layout.npmWorkspaceRoot);
    }
    if (layout.packageManifest) {
        roots.add(dirname(layout.packageManifest));
    }
    if (layout.kind === "hybrid") {
        const packagesDir = join(layout.root, "projects", "packages");
        if (existsSync(packagesDir)) {
            for (const entry of readdirSync(packagesDir, { withFileTypes: true })) {
                if (!entry.isDirectory()) {
                    continue;
                }
                const packageJson = join(packagesDir, entry.name, "package.json");
                if (existsSync(packageJson)) {
                    roots.add(join(packagesDir, entry.name));
                }
            }
        }
    }

    return [...roots];
}
