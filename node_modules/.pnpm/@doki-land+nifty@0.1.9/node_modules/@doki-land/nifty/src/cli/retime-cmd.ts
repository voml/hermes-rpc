import type { Cli, ParsedOptions } from "@vmz/commander";

import { loadNiftyNative } from "../native.js";
import { bootstrapFromOptions } from "./context.js";
import { cwdFrom, str } from "./options.js";

export function registerRetimeCommand(cli: Cli): void {
    const retime = cli.command("retime", "cli.cmd.retime")
        .option("-s, --start-date <start>", "cli.opt.start-date")
        .option("--end-date <end>", "cli.opt.end-date")
        .option("-b, --branch <branch>", "cli.opt.branch")
        .option("--tip <ref>", "cli.opt.tip");

    retime
        .command("root", "cli.cmd.retime.root")
        .option("-s, --start-date <start>", "cli.opt.start-date")
        .option("--end-date <end>", "cli.opt.end-date")
        .option("-b, --branch <branch>", "cli.opt.branch")
        .option("--tip <ref>", "cli.opt.tip")
        .option("-m, --message <text>", "cli.opt.message")
        .action(async (options) => cmdRetimeRoot(options));

    retime.action(async (options) => cmdRetimeRange(options));
}

export async function cmdRetimeRange(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    const commit = options._[0];
    if (!commit) {
        throw new Error("missing commit hash for range retime");
    }
    const native = loadNiftyNative();
    const report = native.history["retime-range"]({
        cwd: cwdFrom(options),
        commit,
        startDate: str(options, "start-date"),
        endDate: str(options, "end-date"),
        branch: str(options, "branch"),
        tip: str(options, "tip"),
    });
    printRetimeSummary(report);
    return 0;
}

export async function cmdRetimeRoot(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    const native = loadNiftyNative();
    const report = native.history["retime-root"]({
        cwd: cwdFrom(options),
        startDate: str(options, "start-date"),
        endDate: str(options, "end-date"),
        branch: str(options, "branch"),
        tip: str(options, "tip"),
        message: str(options, "message"),
    });
    printRetimeSummary(report);
    return 0;
}

function printRetimeSummary(report: { rewritten: number; branch: string; newTip: string }): void {
    console.log(`retimed ${report.rewritten} commit(s) on branch ${report.branch}; tip ${report.newTip}`);
}
