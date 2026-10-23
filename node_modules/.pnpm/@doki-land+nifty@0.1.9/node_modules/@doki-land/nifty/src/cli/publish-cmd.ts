import type { Cli, ParsedOptions } from "@vmz/commander";

import { loadNiftyNative } from "../native.js";
import { authPayload } from "./authArgs.js";
import { loadConfig } from "../config/loadConfig.js";
import { trustPayloadFromConfig } from "../config/trustPayload.js";
import { bootstrapFromOptions } from "./context.js";
import { printPublishReport } from "./publish.js";
import { authFromOptions, cwdFrom, flag, str, strList } from "./options.js";

export function registerPublishCommand(cli: Cli): void {
    cli.command("publish", "cli.cmd.publish")
        .option("--dry-run", "cli.opt.dry-run")
        .option("--refresh", "cli.opt.refresh")
        .option("--placeholder", "cli.opt.placeholder")
        .option("--package <name>", "cli.opt.package")
        .option("--tag <tag>", "cli.opt.tag")
        .option("--access <access>", "cli.opt.access")
        .option("--version <version>", "cli.opt.version")
        .option("--otp <code>", "cli.opt.otp")
        .option("--totp-secret <secret>", "cli.opt.totp-secret")
        .option("--npm-token <token>", "cli.opt.npm-token")
        .action(async (options) => cmdPublish(options));
}

export async function cmdPublish(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    const { config } = await loadConfig({ cwd: cwdFrom(options), createIfMissing: false });
    const packages = strList(options, "package");
    const fromConfig = config.publish?.packages ?? [];
    const native = loadNiftyNative();
    const report = native.publisher["publish-workspace"]({
        cwd: cwdFrom(options),
        dryRun: flag(options, "dry-run"),
        refresh: flag(options, "refresh"),
        placeholder: flag(options, "placeholder"),
        tag: str(options, "tag"),
        access: str(options, "access") ?? "public",
        version: str(options, "version"),
        ...(packages.length === 1
            ? { only: packages[0] }
            : packages.length > 1
              ? { packages }
              : fromConfig.length > 0
                ? { packages: fromConfig }
                : {}),
        ...trustPayloadFromConfig(config),
        ...authPayload(authFromOptions(options)),
    });
    printPublishReport(report, flag(options, "dry-run"));
    return 0;
}
