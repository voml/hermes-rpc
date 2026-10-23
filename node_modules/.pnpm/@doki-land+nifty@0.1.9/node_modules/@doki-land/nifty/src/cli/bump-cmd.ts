import type { Cli, ParsedOptions } from "@vmz/commander";

import { bumpWorkspace, printBumpReport, type BumpKind, type BumpOptions } from "./bump.js";
import { bootstrapFromOptions } from "./context.js";
import { cwdFrom, flag, str } from "./options.js";

export function registerBumpCommand(cli: Cli): void {
    cli.command("bump", "cli.cmd.bump")
        .option("--dry-run", "cli.opt.dry-run")
        .option("--version, -v <version>", "cli.opt.version")
        .option("--patch", "cli.opt.patch")
        .option("--minor", "cli.opt.minor")
        .option("--major", "cli.opt.major")
        .action(async (options) => cmdBump(options));
}

export async function cmdBump(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    const bumpOptions = bumpOptionsFromParsed(options);
    const report = bumpWorkspace(bumpOptions);
    printBumpReport(report, bumpOptions.dryRun);
    return 0;
}

function bumpOptionsFromParsed(options: ParsedOptions): BumpOptions {
    let version = str(options, "version");
    let kind: BumpKind = "patch";
    if (flag(options, "major")) {
        kind = "major";
    } else if (flag(options, "minor")) {
        kind = "minor";
    } else if (flag(options, "patch")) {
        kind = "patch";
    }
    for (const positional of options._) {
        if (isBumpKind(positional)) {
            kind = positional;
        } else if (positional) {
            version = positional;
        }
    }
    if (version && ! /^\d+\.\d+\.\d+(-[\w.-]+)?$/.test(version)) {
        throw new Error(`invalid version ${version}, expected semver like 0.0.0`);
    }
    return {
        version,
        kind,
        cwd: cwdFrom(options),
        dryRun: flag(options, "dry-run"),
    };
}

function isBumpKind(value: string): value is BumpKind {
    return value === "patch" || value === "minor" || value === "major";
}
