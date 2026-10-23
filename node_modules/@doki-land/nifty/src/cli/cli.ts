import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { createCli } from "@vmz/commander";

import { registerBumpCommand } from "./bump-cmd.js";
import { registerFormatCommand } from "./format-cmd.js";
import { registerChangeLogsCommand } from "./change-logs-cmd.js";
import { registerInstallNativeCommand } from "./install-native-cmd.js";
import { registerLintCommands } from "./lint-cmd.js";
import { registerPublishCommand } from "./publish-cmd.js";
import { registerRetimeCommand } from "./retime-cmd.js";
import { registerCommitCommand } from "./commit-cmd.js";
import { registerTrustCommand } from "./trust-cmd.js";
import { registerUpdateCommand } from "./update-cmd.js";
import { registerUploadCommand } from "./upload-cmd.js";

function buildNiftyCli() {
    const localesRoot = join(dirname(fileURLToPath(import.meta.url)), "../../locales");
    const cli = createCli("nifty")
        .locales(localesRoot)
        .intro("cli.intro")
        .option("-C, --cwd <dir>", "cli.opt.cwd");

    registerUpdateCommand(cli);
    registerFormatCommand(cli);
    registerLintCommands(cli);
    registerUploadCommand(cli);
    registerInstallNativeCommand(cli);
    registerBumpCommand(cli);
    registerPublishCommand(cli);
    registerTrustCommand(cli);
    registerCommitCommand(cli);
    registerRetimeCommand(cli);
    registerChangeLogsCommand(cli);

    return cli;
}

export async function runCli(argv: string[]): Promise<number> {
    try {
        return await buildNiftyCli().parse(argv);
    } catch (error) {
        console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
        return 1;
    }
}
