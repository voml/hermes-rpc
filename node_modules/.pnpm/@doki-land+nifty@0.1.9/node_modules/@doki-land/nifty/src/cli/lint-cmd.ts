import type { Cli, ParsedOptions } from "@vmz/commander";

import { bootstrapFromOptions } from "./context.js";
import { lintCommits, type LintOptions } from "./lint.js";
import { cwdFrom, flag, str, strList } from "./options.js";

export function registerLintCommands(cli: Cli): void {
    registerLintLike(cli, "lint", "cli.cmd.lint", false);
    registerLintLike(cli, "check", "cli.cmd.check", true);
}

function registerLintLike(cli: Cli, name: string, helpId: string, check: boolean): void {
    cli.command(name, helpId)
        .option("--from <ref>", "cli.opt.from")
        .option("--to <ref>", "cli.opt.to")
        .option("-s, --subject <text>...", "cli.opt.subject")
        .option("--no-cargo", "cli.opt.no-cargo")
        .action(async (options) => cmdLint(options, check));
}

export async function cmdLint(options: ParsedOptions, check: boolean): Promise<number> {
    await bootstrapFromOptions(options);
    const lintOptions: LintOptions = {
        subjects: strList(options, "subject"),
        from: str(options, "from"),
        to: str(options, "to"),
        cwd: cwdFrom(options),
        scanCargo: flag(options, "no-cargo") ? false : undefined,
    };
    const errorCount = await lintCommits(lintOptions, check);
    if (check && errorCount > 0) {
        throw new Error(`found ${errorCount} lint error(s)`);
    }
    return 0;
}
