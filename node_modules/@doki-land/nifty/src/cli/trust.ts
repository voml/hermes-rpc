import { loadNiftyNative } from "../native.js";
import { authPayload, parseAuthArgs } from "./authArgs.js";

export type TrustReport = {
    root: string;
    configured: string[];
    skipped: string[];
    failed: string[];
};

export async function runTrust(argv: string[]): Promise<void> {
    const options = parseTrustArgs(argv);
    const native = loadNiftyNative();
    const report = native.publisher["trust-workspace"]({
        cwd: options.cwd,
        dryRun: options.dryRun,
        refresh: options.refresh,
        only: options.only,
        ...authPayload(options.auth),
    });
    printTrustReport(report);
}

function parseTrustArgs(argv: string[]) {
    let cwd: string | undefined;
    let dryRun = false;
    let refresh = false;
    let only: string | undefined;
    const auth = parseAuthArgs(argv);

    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === "--dry-run") {
            dryRun = true;
        } else if (arg === "--refresh") {
            refresh = true;
        } else if (arg === "-C" || arg === "--cwd") {
            cwd = argv[++i];
        } else if (arg === "--only") {
            only = argv[++i];
        } else if (arg === "-h" || arg === "--help") {
            printTrustHelp();
            process.exit(0);
        }
    }

    return { cwd, dryRun, refresh, only, auth };
}

export function printTrustReport(report: TrustReport): void {
    console.log(`trust workspace ${report.root}`);
    console.log(`  configured: ${report.configured.length}`);
    for (const name of report.configured) {
        console.log(`    + ${name}`);
    }
    console.log(`  skipped: ${report.skipped.length}`);
    for (const name of report.skipped) {
        console.log(`    = ${name}`);
    }
    if (report.failed.length > 0) {
        console.log(`  failed: ${report.failed.length}`);
        for (const name of report.failed) {
            console.log(`    ! ${name}`);
        }
    }
}

function printTrustHelp(): void {
    console.log(`nifty trust — configure npm Trusted Publisher for workspace packages

Reads trust.npm from nifty.config.ts (repo / file / environment).
Auth: NPM_TOKEN from env or .env.placeholder.local, or npm login token in project .npmrc / ~/.npmrc.
Optional NPM_TOTP_SECRET / NPM_OTP for 2FA accounts (npm-otp header).
Trust list cache lives in .cache/npm-placeholder.json.
Uses the npm registry API (/-/package/*/trust), not npm trust CLI.

Usage:
  nifty trust
  nifty trust --only @doki-land/nifty
  nifty trust --dry-run
  nifty trust --refresh
  nifty trust --totp-secret <base32>
  nifty trust --otp 123456
  nifty trust -C <cwd>
`);
}
