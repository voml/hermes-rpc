import type { Cli, ParsedOptions } from "@vmz/commander";

import { bootstrapFromOptions } from "./context.js";
import { updateWorkspace } from "./update.js";
import { cwdFrom, flag } from "./options.js";

export function registerUpdateCommand(cli: Cli): void {
    cli.command("update", "cli.cmd.update")
        .option("-i, --interactive", "cli.opt.interactive")
        .action(async (options) => cmdUpdate(options));
}

export async function cmdUpdate(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    await updateWorkspace({
        interactive: flag(options, "interactive"),
        cwd: cwdFrom(options),
    });
    return 0;
}
