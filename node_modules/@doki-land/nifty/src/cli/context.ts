import { resolve } from "node:path";

import type { ParsedOptions } from "@vmz/commander";

import { loadConfig } from "../config/loadConfig.js";
import { cwdFrom } from "./options.js";

/** Parse shared CLI flags used across nifty commands. */
export function parseCliContext(argv: string[]): { cwd: string; dryRun: boolean } {
    let cwd = process.cwd();
    let dryRun = false;

    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === "--dry-run") {
            dryRun = true;
        } else if (arg === "-C" || arg === "--cwd") {
            cwd = resolve(argv[++i] ?? cwd);
        }
    }

    return { cwd, dryRun };
}

/** Ensure `nifty.config.*` exists before running a command (skip create on `--dry-run`). */
export async function bootstrapNiftyConfig(argv: string[]): Promise<void> {
    const { cwd, dryRun } = parseCliContext(argv);
    await loadConfig({ cwd, createIfMissing: !dryRun });
}

/** Commander action hook: load config from parsed global/command flags. */
export async function bootstrapFromOptions(options: ParsedOptions): Promise<void> {
    const cwd = cwdFrom(options) ?? process.cwd();
    const dryRun = options["dry-run"] === true;
    await loadConfig({ cwd, createIfMissing: !dryRun });
}
