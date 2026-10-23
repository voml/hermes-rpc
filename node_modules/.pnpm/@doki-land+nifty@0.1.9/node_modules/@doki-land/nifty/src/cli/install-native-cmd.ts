import type { Cli, ParsedOptions } from "@vmz/commander";

import { installNativeFromOptions } from "./install-native.js";
import { cwdFrom, str } from "./options.js";

export function registerInstallNativeCommand(cli: Cli): void {
    cli.command("install-native", "cli.cmd.install-native")
        .option("--from <dir>", "cli.opt.from-artifacts")
        .action(async (options) => cmdInstallNative(options));
}

export async function cmdInstallNative(options: ParsedOptions): Promise<number> {
    const from =
        str(options, "from") ??
        process.env.NATIVE_ARTIFACT_ROOT ??
        "native-artifacts";
    installNativeFromOptions({
        from,
        cwd: cwdFrom(options),
    });
    return 0;
}
