import { loadNiftyNative } from "../native.js";
import { authPayload, parseAuthArgs } from "./authArgs.js";

export type PublishOptions = {
    cwd?: string;
    dryRun?: boolean;
    refresh?: boolean;
    placeholder?: boolean;
    tag?: string;
    access?: string;
    version?: string;
};

export type PublishReport = {
    root: string;
    order: string[];
    published: string[];
    skipped: string[];
    skippedVersions: string[];
};

export async function runPublish(argv: string[]): Promise<void> {
    const options = parsePublishArgs(argv);
    const native = loadNiftyNative();
    const report = native.publisher["publish-workspace"]({
        cwd: options.cwd,
        dryRun: options.dryRun,
        refresh: options.refresh,
        placeholder: options.placeholder,
        tag: options.tag,
        access: options.access,
        version: options.version,
        ...authPayload(options.auth),
    });
    printPublishReport(report, options.dryRun);
}

function parsePublishArgs(argv: string[]) {
    let cwd: string | undefined;
    let dryRun = false;
    let refresh = false;
    let placeholder = false;
    let tag: string | undefined;
    let access = "public";
    let version: string | undefined;
    const auth = parseAuthArgs(argv);

    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === "--dry-run") {
            dryRun = true;
        } else if (arg === "--refresh") {
            refresh = true;
        } else if (arg === "--placeholder") {
            placeholder = true;
        } else if (arg === "-C" || arg === "--cwd") {
            cwd = argv[++i];
        } else if (arg === "--tag") {
            tag = argv[++i];
        } else if (arg === "--access") {
            access = argv[++i];
        } else if (arg === "--version") {
            version = argv[++i];
        } else if (arg === "-h" || arg === "--help") {
            printPublishHelp();
            process.exit(0);
        } else if (arg.startsWith("-")) {
            continue;
        }
    }

    return { cwd, dryRun, refresh, placeholder, tag, access, version, auth };
}

export function printPublishReport(report: PublishReport, dryRun?: boolean): void {
    const prefix = dryRun ? "would publish" : "published";
    console.log(`${prefix} workspace ${report.root}`);
    if (report.skipped.length > 0) {
        console.log(`skipped private packages: ${report.skipped.join(", ")}`);
    }
    if (report.skippedVersions.length > 0) {
        console.log(`skipped already published: ${report.skippedVersions.join(", ")}`);
    }
    console.log("publish order:");
    for (const name of report.order) {
        const marker = report.published.includes(name) ? prefix : "pending";
        console.log(`  ${marker}: ${name}`);
    }
}

function printPublishHelp(): void {
    console.log(`nifty publish — publish workspace npm packages in dependency order

OTP/TOTP is generated in nifty-publisher (set NPM_TOTP_SECRET in .env.placeholder.local).
Publish state is cached in .cache/npm-placeholder.json.

Usage:
  nifty publish
  nifty publish --dry-run
  nifty publish --refresh
  nifty publish --placeholder
  nifty publish --tag next
  nifty publish --access public
  nifty publish --version 0.3.1
  nifty publish --otp 123456
  nifty publish -C <cwd>

--placeholder publishes publishable workspace packages (non-private) with no registry
version yet as 0.0.0 (name claim / Trusted Publisher prep). Packages already on npm
are skipped. Remove private from package.json for packages you intend to ship.
`);
}
