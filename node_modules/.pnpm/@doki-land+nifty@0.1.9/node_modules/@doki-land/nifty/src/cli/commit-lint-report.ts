export type CommitLintDiagnostic = {
    rule: string;
    severity: "error" | "warning" | "info";
    message: string;
    subject?: string;
    hash?: string;
    path?: string;
    line?: number;
};

export type CommitLintReportOptions = {
    json?: boolean;
    errorsOnly?: boolean;
    title?: string;
};

type CommitLintGroup = {
    hash?: string;
    subject?: string;
    items: CommitLintDiagnostic[];
};

export function printCommitLintReport(
    diagnostics: CommitLintDiagnostic[],
    errorCount: number,
    warningCount: number,
    options: CommitLintReportOptions = {},
): void {
    const filtered = options.errorsOnly
        ? diagnostics.filter((item) => item.severity === "error")
        : diagnostics;

    if (options.json) {
        const payload = {
            errorCount,
            warningCount,
            diagnostics: filtered,
            groups: groupCommitDiagnostics(filtered),
            summaryByRule: summarizeByRule(filtered),
        };
        console.log(JSON.stringify(payload, null, 2));
        return;
    }

    const title = options.title ?? "commit lint";
    if (filtered.length === 0) {
        console.log(`${title}: no issues found`);
        return;
    }

    const groups = groupCommitDiagnostics(filtered);
    const commitCount = groups.filter((group) => group.hash || group.subject).length;
    console.log(`${title}: ${errorCount} error(s), ${warningCount} warning(s) in ${commitCount} commit(s)\n`);

    for (const group of groups) {
        printCommitGroup(group);
    }

    const summary = summarizeByRule(filtered.filter((item) => item.severity === "error"));
    if (summary.length > 0) {
        console.log("summary by rule:");
        for (const [rule, count] of summary) {
            console.log(`  ${rule}  ${count}`);
        }
        console.log();
    }
}

function groupCommitDiagnostics(diagnostics: CommitLintDiagnostic[]): CommitLintGroup[] {
    const groups = new Map<string, CommitLintGroup>();
    const order: string[] = [];

    for (const item of diagnostics) {
        const key = item.hash ?? item.subject ?? "__inline__";
        if (!groups.has(key)) {
            groups.set(key, {
                hash: item.hash,
                subject: item.subject,
                items: [],
            });
            order.push(key);
        }
        groups.get(key)?.items.push(item);
    }

    return order.map((key) => groups.get(key)!);
}

function summarizeByRule(diagnostics: CommitLintDiagnostic[]): Array<[string, number]> {
    const counts = new Map<string, number>();
    for (const item of diagnostics) {
        counts.set(item.rule, (counts.get(item.rule) ?? 0) + 1);
    }
    return [...counts.entries()].sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]));
}

function printCommitGroup(group: CommitLintGroup): void {
    if (group.hash || group.subject) {
        const hash = group.hash ? `${group.hash.slice(0, 8)}  ` : "";
        const subject = group.subject ?? "(no subject)";
        console.log(`${hash}${subject}`);
    } else {
        console.log("(inline subject lint)");
    }

    for (const item of group.items) {
        console.log(`  ${item.severity}  ${item.rule}  ${item.message}`);
    }
    console.log();
}
