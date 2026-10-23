import type { Cli, ParsedOptions } from "@vmz/commander";

import { loadNiftyNative } from "../native.js";
import { authPayload } from "./authArgs.js";
import { loadConfig } from "../config/loadConfig.js";
import { trustPayloadFromConfig } from "../config/trustPayload.js";
import { bootstrapFromOptions } from "./context.js";
import { printTrustReport } from "./trust.js";
import { authFromOptions, cwdFrom, flag, str } from "./options.js";

export function registerTrustCommand(cli: Cli): void {
    cli.command("trust", "cli.cmd.trust")
        .option("--dry-run", "cli.opt.dry-run")
        .option("--refresh", "cli.opt.refresh")
        .option("--only <package>", "cli.opt.only")
        .option("--otp <code>", "cli.opt.otp")
        .option("--totp-secret <secret>", "cli.opt.totp-secret")
        .option("--npm-token <token>", "cli.opt.npm-token")
        .action(async (options) => cmdTrust(options));
}

export async function cmdTrust(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    const { config } = await loadConfig({ cwd: cwdFrom(options), createIfMissing: false });
    const native = loadNiftyNative();
    const fromConfig = config.publish?.packages ?? [];
    const report = native.publisher["trust-workspace"]({
        cwd: cwdFrom(options),
        dryRun: flag(options, "dry-run"),
        refresh: flag(options, "refresh"),
        only: str(options, "only"),
        ...(fromConfig.length > 0 ? { packages: fromConfig } : {}),
        ...trustPayloadFromConfig(config),
        ...authPayload(authFromOptions(options)),
    });
    printTrustReport(report);
    if (report.failed.length > 0) {
        throw new Error(`trust failed for ${report.failed.length} package(s)`);
    }
    return 0;
}
