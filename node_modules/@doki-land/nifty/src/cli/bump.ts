import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { findWorkspaceRoot } from "./workspace.js";

export type BumpKind = "patch" | "minor" | "major";

export type BumpOptions = {
    /** Explicit target version. When set, overrides `kind`. */
    version?: string;
    kind?: BumpKind;
    cwd?: string;
    dryRun?: boolean;
};

export type BumpReport = {
    root: string;
    from: string;
    to: string;
    kind?: BumpKind;
    cargo: string[];
    npm: string[];
};

const DEFAULT_KIND: BumpKind = "patch";

export async function runBump(argv: string[]): Promise<void> {
    const options = parseBumpArgs(argv);
    const report = bumpWorkspace(options);
    printBumpReport(report, options.dryRun);
}

function parseBumpArgs(argv: string[]): BumpOptions {
    let version: string | undefined;
    let kind: BumpKind = DEFAULT_KIND;
    let cwd: string | undefined;
    let dryRun = false;

    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === "--dry-run") {
            dryRun = true;
        } else if (arg === "-C" || arg === "--cwd") {
            cwd = argv[++i];
        } else if (arg === "--version" || arg === "-v") {
            version = argv[++i];
        } else if (arg === "--patch") {
            kind = "patch";
        } else if (arg === "--minor") {
            kind = "minor";
        } else if (arg === "--major") {
            kind = "major";
        } else if (arg === "-h" || arg === "--help") {
            printBumpHelp();
            process.exit(0);
        } else if (!arg.startsWith("-")) {
            if (isBumpKind(arg)) {
                kind = arg;
            } else {
                version = arg;
            }
        }
    }

    if (version) {
        assertVersion(version);
    }
    return { version, kind, cwd, dryRun };
}

function isBumpKind(value: string): value is BumpKind {
    return value === "patch" || value === "minor" || value === "major";
}

function assertVersion(version: string): void {
    if (! /^\d+\.\d+\.\d+(-[\w.-]+)?$/.test(version)) {
        throw new Error(`invalid version ${version}, expected semver like 0.0.0`);
    }
}

export function bumpWorkspace(options: BumpOptions): BumpReport {
    const root = findWorkspaceRoot(options.cwd ?? process.cwd());
    const cratesDir = join(root, "projects", "crates");
    const packagesDir = join(root, "projects", "packages");

    const cargoPaths = collectCargoManifests(cratesDir);
    const npmPaths = collectPackageManifests(packagesDir);
    const from = readWorkspaceVersion(root, cargoPaths, npmPaths);
    const to = options.version ?? bumpSemver(from, options.kind ?? DEFAULT_KIND);
    assertVersion(to);

    const cargoUpdates = prepareCargoUpdates(root, cargoPaths, to);
    const npmUpdates = npmPaths.map((path) => preparePackageUpdate(path, to));
    if (!options.dryRun) {
        cargoUpdates.concat(npmUpdates).forEach((update) => writeFileSync(update.path, update.next, "utf8"));
    }

    return {
        root,
        from,
        to,
        kind: options.version ? undefined : options.kind ?? DEFAULT_KIND,
        cargo: cargoUpdates.map((update) => update.path),
        npm: npmUpdates.map((update) => update.path),
    };
}

function readWorkspaceVersion(root: string, cargoPaths: string[], npmPaths: string[]): string {
    const versions = new Set<string>();
    let usesWorkspaceVersion = false;
    for (const path of cargoPaths) {
        const info = readCargoVersion(path);
        if (info.inherited) usesWorkspaceVersion = true;
        else versions.add(info.version);
    }
    if (usesWorkspaceVersion) {
        versions.add(readWorkspacePackageVersion(join(root, "Cargo.toml")).version);
    }
    for (const path of npmPaths) {
        versions.add(readPackageVersion(path));
    }
    if (versions.size === 0) {
        throw new Error("no crate or package versions found in workspace");
    }
    if (versions.size > 1) {
        throw new Error(`workspace versions are out of sync: ${[...versions].sort().join(", ")}`);
    }
    return [...versions][0];
}

type CargoVersion = { version: string; inherited: boolean };

type FileUpdate = { path: string; next: string };

function readCargoVersion(path: string): CargoVersion {
    const original = readFileSync(path, "utf8");
    const field = findTomlField(original, "package", "version.workspace") ?? findTomlField(original, "package", "version");
    if (!field) throw new Error(`no [package].version found in ${path}`);
    if (isWorkspaceInheritedVersion(field)) {
        return { version: "", inherited: true };
    }
    return { version: parseTomlString(field.value, `[package].version in ${path}`), inherited: false };
}

function readPackageVersion(path: string): string {
    const parsed = JSON.parse(readFileSync(path, "utf8")) as { version?: string };
    if (!parsed.version) {
        throw new Error(`no version field in ${path}`);
    }
    return parsed.version;
}

export function bumpSemver(version: string, kind: BumpKind): string {
    const match = version.match(/^(\d+)\.(\d+)\.(\d+)(?:-.*)?$/);
    if (!match) {
        throw new Error(`cannot bump non-semver version ${version}`);
    }
    let major = Number(match[1]);
    let minor = Number(match[2]);
    let patch = Number(match[3]);

    switch (kind) {
        case "patch":
            patch += 1;
            break;
        case "minor":
            minor += 1;
            patch = 0;
            break;
        case "major":
            major += 1;
            minor = 0;
            patch = 0;
            break;
    }

    return `${major}.${minor}.${patch}`;
}

function collectCargoManifests(cratesDir: string): string[] {
    if (!existsSync(cratesDir)) {
        return [];
    }
    return readdirSync(cratesDir, { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .map((entry) => join(cratesDir, entry.name, "Cargo.toml"))
        .filter((path) => existsSync(path));
}

function collectPackageManifests(packagesDir: string): string[] {
    if (!existsSync(packagesDir)) {
        return [];
    }
    return readdirSync(packagesDir, { withFileTypes: true })
        .filter((entry) => entry.isDirectory() && entry.name !== "node_modules")
        .map((entry) => join(packagesDir, entry.name, "package.json"))
        .filter((path) => existsSync(path));
}

function prepareCargoUpdates(root: string, cargoPaths: string[], version: string): FileUpdate[] {
    const updates: FileUpdate[] = [];
    let usesWorkspaceVersion = false;
    for (const path of cargoPaths) {
        const original = readFileSync(path, "utf8");
        const field = findTomlField(original, "package", "version.workspace") ?? findTomlField(original, "package", "version");
        if (!field) throw new Error(`no [package].version found in ${path}`);
        if (isWorkspaceInheritedVersion(field)) {
            usesWorkspaceVersion = true;
            continue;
        }
        parseTomlString(field.value, `[package].version in ${path}`);
        updates.push({ path, next: replaceTomlField(original, field, version) });
    }
    if (usesWorkspaceVersion) {
        const path = join(root, "Cargo.toml");
        const original = readFileSync(path, "utf8");
        const field = findTomlField(original, "workspace.package", "version");
        if (!field) {
            throw new Error(`Cargo members inherit version.workspace but ${path} has no [workspace.package].version`);
        }
        parseTomlString(field.value, `[workspace.package].version in ${path}`);
        updates.push({ path, next: replaceTomlField(original, field, version) });
    }
    return updates;
}

function readWorkspacePackageVersion(path: string): CargoVersion {
    const original = readFileSync(path, "utf8");
    const field = findTomlField(original, "workspace.package", "version");
    if (!field) throw new Error(`Cargo members inherit version.workspace but ${path} has no [workspace.package].version`);
    return { version: parseTomlString(field.value, `[workspace.package].version in ${path}`), inherited: false };
}

function isWorkspaceInheritedVersion(field: NonNullable<ReturnType<typeof findTomlField>>): boolean {
    if (field.key === "version.workspace") return true;
    const value = field.value.trim();
    if (!value.startsWith("{") || !value.endsWith("}")) return false;
    return value
        .slice(1, -1)
        .split(",")
        .some((assignment) => {
            const equals = assignment.indexOf("=");
            return equals >= 0 && assignment.slice(0, equals).trim() === "workspace" && assignment.slice(equals + 1).trim() === "true";
        });
}

function findTomlField(contents: string, table: string, name: string) {
    let currentTable = "";
    const lines = contents.split(/(?<=\n)/);
    for (let index = 0; index < lines.length; index += 1) {
        const line = lines[index];
        const trimmed = line.trim();
        const tableMatch = trimmed.match(/^\[([^\]]+)\]$/);
        if (tableMatch) {
            currentTable = tableMatch[1].trim();
            continue;
        }
        if (trimmed.startsWith("[[")) {
            currentTable = "";
            continue;
        }
        if (currentTable !== table) continue;
        const fieldMatch = line.match(new RegExp(`^(\\s*)(${name.replace(".", "\\.")})\\s*=\\s*(.*?)(\\r?\\n?)$`));
        if (fieldMatch) return { index, line, indent: fieldMatch[1], key: fieldMatch[2], value: fieldMatch[3], ending: fieldMatch[4] };
    }
    return undefined;
}

function parseTomlString(value: string, description: string): string {
    const trimmed = value.trim();
    const quote = trimmed[0];
    if (quote !== '"' && quote !== "'") throw new Error(`expected a quoted string for ${description}`);
    const closingQuote = trimmed.indexOf(quote, 1);
    if (closingQuote < 0) throw new Error(`expected a quoted string for ${description}`);
    const trailing = trimmed.slice(closingQuote + 1).trim();
    if (trailing && !trailing.startsWith("#")) throw new Error(`expected a quoted string for ${description}`);
    return trimmed.slice(1, closingQuote);
}

function replaceTomlField(contents: string, field: NonNullable<ReturnType<typeof findTomlField>>, version: string): string {
    const value = field.value.trimStart();
    const closingQuote = value.indexOf(value[0], 1);
    const suffix = closingQuote >= 0 ? value.slice(closingQuote + 1) : "";
    const line = `${field.indent}${field.key} = "${version}"${suffix}${field.ending}`;
    return contents.split(/(?<=\n)/).map((current, index) => index === field.index ? line : current).join("");
}

function preparePackageUpdate(path: string, version: string): FileUpdate {
    const original = readFileSync(path, "utf8");
    const parsed = JSON.parse(original) as { version?: string };
    if (!parsed.version) {
        throw new Error(`no version field in ${path}`);
    }
    parsed.version = version;
    const next = `${JSON.stringify(parsed, null, 4)}\n`;
    return { path, next };
}

export function printBumpReport(report: BumpReport, dryRun?: boolean): void {
    const prefix = dryRun ? "would bump" : "bumped";
    const kind = report.kind ? ` (${report.kind})` : "";
    console.log(`${prefix} workspace ${report.root} ${report.from} -> ${report.to}${kind}`);
    for (const path of report.cargo) {
        console.log(`  cargo: ${path}`);
    }
    for (const path of report.npm) {
        console.log(`  npm: ${path}`);
    }
}

function printBumpHelp(): void {
    console.log(`nifty bump — align all crate and package versions in the workspace

Usage:
  nifty bump                 # bump patch (default)
  nifty bump patch
  nifty bump minor
  nifty bump major
  nifty bump 1.2.3           # set explicit version
  nifty bump --version 1.2.3
  nifty bump --dry-run
  nifty bump -C <cwd>

Cargo members using version.workspace = true are updated through the root [workspace.package].version.
`);
}
