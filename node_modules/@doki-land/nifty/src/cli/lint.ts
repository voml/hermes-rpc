import { loadConfig } from "../config/loadConfig.js";
import { loadNiftyNative } from "../native.js";
import { printCommitLintReport, type CommitLintDiagnostic } from "./commit-lint-report.js";

export type LintOptions = {
    subjects: string[];
    from?: string;
    to?: string;
    cwd?: string;
    scanCargo?: boolean;
    commitOnly?: boolean;
    json?: boolean;
    errorsOnly?: boolean;
    reportTitle?: string;
};

type LintDiagnostic = CommitLintDiagnostic;

type LintRuleConfig = {
    id: string;
    enabled?: boolean;
    severity?: "error" | "warning" | "info";
};

export async function runLint(argv: string[], check: boolean): Promise<void> {
    const options = parseLintArgs(argv);
    const errorCount = await lintCommits(options, check);
    if (check && errorCount > 0) {
        throw new Error(`found ${errorCount} lint error(s)`);
    }
}

export async function lintCommits(
    options: LintOptions,
    check: boolean,
    commitOnly = options.commitOnly ?? false,
): Promise<number> {
    const native = loadNiftyNative();
    const cwd = options.cwd ?? process.cwd();
    const { config } = await loadConfig({ cwd, createIfMissing: false });
    const rules: LintRuleConfig[] | undefined = config.lint?.rules?.map((rule) => ({
        id: rule.id,
        enabled: rule.enabled,
        severity: rule.severity,
    }));

    const runner = check ? native.lint.check : native.lint.run;
    const report = runner({
        cwd,
        fromRef: options.from,
        toRef: options.to,
        subjects: options.subjects.length > 0 ? options.subjects : undefined,
        rules,
        scanCargo: options.scanCargo,
        commitOnly: commitOnly || options.commitOnly,
    });

    if (commitOnly || options.commitOnly) {
        printCommitLintReport(report.diagnostics, report.errorCount, report.warningCount, {
            json: options.json,
            errorsOnly: options.errorsOnly,
            title: options.reportTitle ?? "commit lint",
        });
    } else {
        for (const item of report.diagnostics) {
            printWorkspaceDiagnostic(item);
        }

        if (report.diagnostics.length > 0) {
            console.log(`lint: ${report.errorCount} error(s), ${report.warningCount} warning(s)`);
        } else {
            console.log("lint: no issues found");
        }
    }

    return report.errorCount;
}

function printWorkspaceDiagnostic(item: LintDiagnostic): void {
    const prefix = item.severity === "error" ? "error" : item.severity === "warning" ? "warning" : "info";
    if (item.subject) {
        const hash = item.hash ? `${item.hash.slice(0, 8)} ` : "";
        console.log(`${prefix} [${item.rule}] ${hash}${item.message} (${item.subject})`);
        return;
    }
    const location =
        item.path !== undefined
            ? item.line !== undefined
                ? `${item.path}:${item.line}`
                : item.path
            : undefined;
    const suffix = location ? ` @ ${location}` : "";
    console.log(`${prefix} [${item.rule}] ${item.message}${suffix}`);
}

function parseLintArgs(argv: string[]): LintOptions {
    const options: LintOptions = { subjects: [] };
    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === "--from") {
            options.from = argv[++i];
        } else if (arg === "--to") {
            options.to = argv[++i];
        } else if (arg === "-C" || arg === "--cwd") {
            options.cwd = argv[++i];
        } else if (arg === "-s" || arg === "--subject") {
            options.subjects.push(argv[++i]);
        } else if (arg === "--no-cargo") {
            options.scanCargo = false;
        }
    }
    return options;
}
