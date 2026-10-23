import { existsSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { createJiti } from "jiti";

import type { NiftyConfig, NiftyConfigEnv, NiftyConfigExport } from "./types.js";
import { CONFIG_FILE_NAMES } from "./types.js";
import { detectProjectLayout } from "./detectProject.js";

export type LoadConfigOptions = {
    /** Directory to start searching from (defaults to `process.cwd()`). */
    cwd?: string;
    /** When set, skip searching and load this file directly. */
    configFile?: string;
    /** Passed to functional configs: `export default defineConfig(({ mode }) => ...)`. */
    env?: Partial<NiftyConfigEnv>;
    /**
     * When true (default), write `nifty.config.ts` at the detected project root if missing.
     * Set false for `--dry-run` so commands only report what would happen.
     */
    createIfMissing?: boolean;
};

export type LoadedNiftyConfig = {
    config: NiftyConfig;
    configFile?: string;
    /** True when this run created a new config file. */
    created?: boolean;
};

const DEFAULT_CONFIG_TEMPLATE = `// Nifty project configuration.
// See @doki-land/nifty for defineConfig, format presets, authorMap path, and lint rules.
export default {};
`;

const DEFAULT_ENV = (cwd: string): NiftyConfigEnv => ({
    mode: process.env.NODE_ENV ?? "development",
    command: "serve",
    layout: detectProjectLayout(cwd),
});

/** Walk upward and return the nearest existing Nifty config file. */
export function findConfigFile(cwd = process.cwd()): string | undefined {
    let here = resolve(cwd);
    while (true) {
        for (const name of CONFIG_FILE_NAMES) {
            const candidate = join(here, name);
            if (existsSync(candidate)) {
                return candidate;
            }
        }
        const parent = dirname(here);
        if (parent === here) {
            return undefined;
        }
        here = parent;
    }
}

/** Default config path at the detected project root (prefers `nifty.config.ts`). */
export function defaultConfigPath(cwd = process.cwd()): string {
    const layout = detectProjectLayout(cwd);
    return join(layout.root, CONFIG_FILE_NAMES[0]);
}

/** Warn and optionally create a default config when none exists. */
export function ensureConfigFile(options: { cwd?: string; create?: boolean } = {}): string | undefined {
    const cwd = options.cwd ?? process.cwd();
    const existing = findConfigFile(cwd);
    if (existing) {
        return existing;
    }

    const target = defaultConfigPath(cwd);
    if (options.create) {
        writeFileSync(target, DEFAULT_CONFIG_TEMPLATE, "utf8");
        console.warn(`nifty: no config found, created default ${target}`);
        return target;
    }

    console.warn(`nifty: no nifty.config.* found near ${detectProjectLayout(cwd).root} (using defaults)`);
    return undefined;
}

function normalizeExport(raw: unknown): NiftyConfigExport {
    if (raw && typeof raw === "object" && "default" in raw) {
        return (raw as { default: NiftyConfigExport }).default;
    }
    return raw as NiftyConfigExport;
}

async function resolveExport(exported: NiftyConfigExport, env: NiftyConfigEnv): Promise<NiftyConfig> {
    if (typeof exported === "function") {
        return await exported(env);
    }
    return exported;
}

/** Load `nifty.config.ts/js`, creating a default file at the project root when missing. */
export async function loadConfig(options: LoadConfigOptions = {}): Promise<LoadedNiftyConfig> {
    const cwd = options.cwd ?? process.cwd();
    let configFile = options.configFile ?? findConfigFile(cwd);
    let created = false;

    if (!configFile && options.createIfMissing !== false) {
        configFile = ensureConfigFile({ cwd, create: true });
        created = Boolean(configFile);
    } else if (!configFile) {
        ensureConfigFile({ cwd, create: false });
        return { config: {} };
    }

    if (!configFile) {
        return { config: {} };
    }

    const env: NiftyConfigEnv = { ...DEFAULT_ENV(cwd), ...options.env };
    const jiti = createJiti(import.meta.url, { interopDefault: true });
    const mod = await jiti.import(configFile);
    const config = await resolveExport(normalizeExport(mod), env);
    return { config, configFile, created };
}

