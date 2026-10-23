import { readFile, writeFile } from "node:fs/promises";

import type { Cli, ParsedOptions } from "@vmz/commander";

import { loadNiftyNative } from "../native.js";
import { bootstrapFromOptions } from "./context.js";
import { lintCommits, type LintOptions } from "./lint.js";
import { cwdFrom, flag, str, strList } from "./options.js";

export function registerCommitCommand(cli: Cli): void {
    const commit = cli.command("commit", "cli.cmd.commit");

    commit
        .command("export", "cli.cmd.commit.export")
        .option("--base <ref>", "cli.opt.base")
        .option("--ref <ref>", "cli.opt.ref")
        .option("--path <file>", "cli.opt.path")
        .option("--with-lint", "cli.opt.with-lint")
        .option("--errors-only", "cli.opt.errors-only")
        .action(async (options) => cmdCommitExport(options));

    commit
        .command("apply", "cli.cmd.commit.apply")
        .option("--base <ref>", "cli.opt.base")
        .option("--ref <ref>", "cli.opt.ref")
        .option("--path <file>", "cli.opt.path")
        .option("--dry-run", "cli.opt.dry-run")
        .action(async (options) => cmdCommitApply(options));

    commit
        .command("scan", "cli.cmd.commit.scan")
        .option("--from <ref>", "cli.opt.from")
        .option("--to <ref>", "cli.opt.to")
        .option("-s, --subject <text>...", "cli.opt.subject")
        .option("--json", "cli.opt.json")
        .option("--errors-only", "cli.opt.errors-only")
        .action(async (options) => cmdCommitScan(options));

    commit
        .command("audit", "cli.cmd.commit.audit")
        .option("--from <ref>", "cli.opt.from")
        .option("--to <ref>", "cli.opt.to")
        .option("-s, --subject <text>...", "cli.opt.subject")
        .option("--json", "cli.opt.json")
        .option("--errors-only", "cli.opt.errors-only")
        .action(async (options) => cmdCommitAudit(options));
}

export async function cmdCommitExport(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    const base = requireStr(options, "base");
    const path = str(options, "path") ?? "commit.pending.json";
    const native = loadNiftyNative();
    const report = native.history["commit-export"]({
        cwd: cwdFrom(options),
        base,
        ref: str(options, "ref"),
        path,
    });
    console.log(`export: wrote ${report.count} commit entry(ies) to ${report.path}`);

    if (flag(options, "with-lint")) {
        const errorCount = await lintAndAnnotateExport({
            path,
            from: base,
            to: str(options, "ref") ?? "HEAD",
            cwd: cwdFrom(options) ?? process.cwd(),
            json: flag(options, "json"),
            errorsOnly: flag(options, "errors-only"),
            reportTitle: "commit export lint",
        });
        if (errorCount > 0) {
            return 1;
        }
    }

    return 0;
}

export async function cmdCommitApply(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    const base = requireStr(options, "base");
    const path = requireStr(options, "path");
    const dryRun = flag(options, "dry-run");
    const native = loadNiftyNative();
    const report = native.history["commit-apply"]({
        cwd: cwdFrom(options),
        base,
        ref: str(options, "ref"),
        path,
        dryRun,
    });

    if (dryRun) {
        if (report.changes.length === 0) {
            console.log("dry-run: no commit objects need rewriting");
            return 0;
        }
        console.log(`dry-run: ${report.changes.length} commit object(s) would be rewritten on ${report.refName}\n`);
        for (const change of report.changes) {
            console.log(`${change.oldOid}  ${change.oldSubject}`);
            console.log(`     ->  ${change.newSubject}`);
            if (change.parentsRelinked && !change.messageChanged) {
                console.log("     (parent chain relink only)");
            }
            console.log();
        }
        return 0;
    }

    console.log(
        `applied ${report.changes.length} commit object(s); ${report.oldTip} -> ${report.newTip ?? report.oldTip}`,
    );
    for (const change of report.changes) {
        console.log(`  ${change.oldOid}  ${change.newSubject}`);
    }
    return 0;
}

export async function cmdCommitScan(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    await lintCommits({ ...commitLintOptions(options), reportTitle: "commit scan" }, false, true);
    return 0;
}

export async function cmdCommitAudit(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    const errorCount = await lintCommits(commitLintOptions(options), false, true);
    return errorCount > 0 ? 1 : 0;
}

function commitLintOptions(options: ParsedOptions): LintOptions {
    return {
        subjects: strList(options, "subject"),
        from: str(options, "from"),
        to: str(options, "to"),
        cwd: cwdFrom(options),
        scanCargo: false,
        commitOnly: true,
        json: flag(options, "json"),
        errorsOnly: flag(options, "errors-only"),
        reportTitle: "commit audit",
    };
}

type ExportDocument = {
    version: number;
    entries: Array<{
        hash: string;
        message: string;
        violations?: Array<{
            rule: string;
            severity: "error" | "warning" | "info";
            message: string;
        }>;
    }>;
};

async function lintAndAnnotateExport(options: {
    path: string;
    from: string;
    to: string;
    cwd: string;
    json: boolean;
    errorsOnly: boolean;
    reportTitle: string;
}): Promise<number> {
    const native = loadNiftyNative();
    const report = native.lint.run({
        cwd: options.cwd,
        fromRef: options.from,
        toRef: options.to,
        commitOnly: true,
        scanCargo: false,
    });
    const diagnostics = options.errorsOnly
        ? report.diagnostics.filter((item) => item.severity === "error")
        : report.diagnostics;

    if (options.json) {
        const { printCommitLintReport } = await import("./commit-lint-report.js");
        printCommitLintReport(diagnostics, report.errorCount, report.warningCount, {
            json: true,
            errorsOnly: options.errorsOnly,
            title: options.reportTitle,
        });
    } else {
        const { printCommitLintReport } = await import("./commit-lint-report.js");
        printCommitLintReport(diagnostics, report.errorCount, report.warningCount, {
            errorsOnly: options.errorsOnly,
            title: options.reportTitle,
        });
    }

    const byHash = new Map<string, Array<{ rule: string; severity: "error" | "warning" | "info"; message: string }>>();
    for (const item of diagnostics) {
        if (!item.hash) {
            continue;
        }
        const bucket = byHash.get(item.hash) ?? [];
        bucket.push({ rule: item.rule, severity: item.severity, message: item.message });
        byHash.set(item.hash, bucket);
    }

    const text = await readFile(options.path, "utf8");
    const document = JSON.parse(text) as ExportDocument;
    document.entries = document.entries.map((entry) => {
        const violations = byHash.get(entry.hash);
        if (!violations || violations.length === 0) {
            return entry;
        }
        return { ...entry, violations };
    });
    await writeFile(options.path, `${JSON.stringify(document, null, 2)}\n`, "utf8");
    const violationCount = document.entries.filter((entry) => entry.violations && entry.violations.length > 0).length;
    console.log(`export: annotated ${violationCount} commit entry(ies) with lint violations in ${options.path}`);
    return report.errorCount;
}

function requireStr(options: ParsedOptions, key: string): string {
    const value = str(options, key);
    if (!value) {
        throw new Error(`missing required --${key}`);
    }
    return value;
}
