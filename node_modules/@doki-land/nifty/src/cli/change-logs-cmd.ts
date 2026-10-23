import { join } from "node:path";

import type { Cli, ParsedOptions } from "@vmz/commander";

import { resolveAuthorMapPath } from "../config/authorMap.js";
import { detectProjectLayout } from "../config/detectProject.js";
import { loadConfig } from "../config/loadConfig.js";
import { loadNiftyNative } from "../native.js";
import { bootstrapFromOptions } from "./context.js";
import { cwdFrom, flag, str } from "./options.js";

export function registerChangeLogsCommand(cli: Cli): void {
    const changeLogs = cli.command("change-logs", "cli.cmd.change-logs")
        .option("--version <version>", "cli.opt.version")
        .option("--from <ref>", "cli.opt.from")
        .option("--to <ref>", "cli.opt.to")
        .option("--write", "cli.opt.write")
        .option("--tags", "cli.opt.tags")
        .option("--repo <owner/name>", "cli.opt.repo")
        .option("--author-map <path>", "cli.opt.author-map")
        .option("--releases-dir <path>", "cli.opt.releases-dir");

    changeLogs
        .command("lookup", "cli.cmd.change-logs.lookup")
        .option("--email <email>", "cli.opt.email")
        .option("--login <login>", "cli.opt.login")
        .option("--map <path>", "cli.opt.author-map")
        .option("--github-token <token>", "cli.opt.github-token")
        .option("--fetch", "cli.opt.fetch")
        .action(async (options) => cmdChangeLogsLookup(options));

    changeLogs.action(async (options) => cmdChangeLogsRender(options));
}

export async function cmdChangeLogsRender(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    const cwd = cwdFrom(options) ?? process.cwd();
    const { config } = await loadConfig({ cwd, createIfMissing: false });
    const layout = detectProjectLayout(cwd);
    const repoRoot = config.repoRoot ?? layout.root;
    const native = loadNiftyNative();
    const report = native.history["changelog-render"]({
        cwd,
        version: str(options, "version"),
        fromRef: str(options, "from"),
        toRef: str(options, "to"),
        write: flag(options, "write"),
        tags: flag(options, "tags"),
        repo: str(options, "repo") ?? config.changelog?.repo,
        authorMap: str(options, "author-map") ?? resolveAuthorMapPath(config, repoRoot),
        releasesDir: str(options, "releases-dir") ?? config.changelog?.releasesDir,
    });

    if (flag(options, "tags")) {
        console.log(report.notes);
        return 0;
    }

    if (report.rangeLabel) {
        console.error(`change-logs: ${report.rangeLabel} — ${report.commitCount} commit(s)`);
    }
    if (report.writtenPath) {
        console.error(`change-logs: wrote ${report.writtenPath}`);
    }
    process.stdout.write(report.notes);
    return 0;
}

export async function cmdChangeLogsLookup(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    const cwd = cwdFrom(options) ?? process.cwd();
    const { config } = await loadConfig({ cwd, createIfMissing: false });
    const layout = detectProjectLayout(cwd);
    const repoRoot = config.repoRoot ?? layout.root;
    const email = str(options, "email");
    const login = str(options, "login");
    if (!email && !login) {
        throw new Error("lookup requires --email or --login");
    }
    const native = loadNiftyNative();
    const author = native.history["changelog-lookup"]({
        cwd,
        email,
        login,
        map: str(options, "map") ?? resolveAuthorMapPath(config, repoRoot),
        githubToken: str(options, "github-token") ?? config.githubToken ?? process.env.GITHUB_TOKEN,
        fetch: flag(options, "fetch"),
    });
    const json: Record<string, unknown> = {};
    if (author.id !== undefined) {
        json.id = author.id;
    }
    if (author.login) {
        json.login = author.login;
    }
    console.log(JSON.stringify(json, null, 2));
    return 0;
}

/** Default author map path relative to repo root (matches git-change-logs). */
export function defaultAuthorMapPath(repoRoot: string): string {
    return join(repoRoot, "documentation/maintenance/author-github.json");
}
