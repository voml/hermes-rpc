import { createRequire } from "node:module";
import type {
    CommitRecord,
    GithubAuthor,
    ParsedSubject,
    RangeInfo,
    ReleaseSection,
    TagInfo,
} from "./types.js";

export type GitmojiExports = {
    "known-gitmojis": () => string[];
    "validate-subject": (subject: string) => boolean;
    "parse-subject": (subject: string) => ParsedSubject;
    "format-subject": (gitmoji: string, body: string) => string;
    "strip-gitmoji": (subject: string) => string;
    "leading-gitmoji": (subject: string) => string | undefined;
    "section-for-gitmoji": (gitmoji: string | undefined) => ReleaseSection;
    "parse-noreply-email": (email: string) => GithubAuthor | undefined;
    "resolve-github-author": (email: string, authorMapJson: string) => GithubAuthor | undefined;
    "display-login": (author: GithubAuthor) => string;
    "profile-url": (author: GithubAuthor) => string;
    "avatar-url": (author: GithubAuthor) => string;
    "author-mention": (email: string, authorName: string, authorMapJson: string) => string;
    "commit-bullet": (body: string, email: string, authorName: string, authorMapJson: string) => string;
};

export type GitExports = {
    "discover-root": (startPath: string) => string;
    "list-version-tags": (repoRoot: string) => string[];
    "list-tag-infos": (repoRoot: string) => Array<{ name: string; "short-hash": string }>;
    "format-tag-list": (repoRoot: string) => string;
    "resolve-range": (
        repoRoot: string,
        version: string | undefined,
        fromRef: string | undefined,
        toRef: string | undefined,
    ) => { version: string; "from-ref"?: string; "to-ref": string };
    "collect-commits": (
        repoRoot: string,
        fromRef: string | undefined,
        toRef: string,
    ) => NativeCommitRecord[];
    "detect-github-repo": (repoRoot: string) => string | undefined;
    "parse-github-remote": (url: string) => string | undefined;
};

type NativeCommitRecord = Omit<CommitRecord, "section"> & { section: string };

export type PublishReport = {
    root: string;
    order: string[];
    published: string[];
    skipped: string[];
    skippedVersions: string[];
};

export type TrustReport = {
    root: string;
    configured: string[];
    skipped: string[];
    failed: string[];
};

export type RewordPlannedChange = {
    oldOid: string;
    oldSubject: string;
    newSubject: string;
    parentsRelinked: boolean;
    messageChanged: boolean;
};

export type RewordRewriteReport = {
    changes: RewordPlannedChange[];
    oldTip: string;
    newTip?: string;
    refName: string;
    dryRun: boolean;
};

export type HistoryExports = {
    "commit-export": (options: {
        cwd?: string;
        base: string;
        ref?: string;
        path: string;
    }) => { count: number; path: string };
    "commit-apply": (options: {
        cwd?: string;
        base: string;
        ref?: string;
        path: string;
        dryRun?: boolean;
    }) => RewordRewriteReport;
    /** @deprecated use `commit-export` */
    "reword-export": (options: {
        cwd?: string;
        base: string;
        ref?: string;
        path: string;
    }) => { count: number; path: string };
    /** @deprecated use `commit-apply` */
    "reword-rewrite": (options: {
        cwd?: string;
        base: string;
        ref?: string;
        path: string;
        dryRun?: boolean;
    }) => RewordRewriteReport;
    "retime-range": (options: {
        cwd?: string;
        commit: string;
        startDate?: string;
        endDate?: string;
        branch?: string;
        tip?: string;
    }) => { branch: string; newTip: string; rewritten: number };
    "retime-root": (options: {
        cwd?: string;
        startDate?: string;
        endDate?: string;
        branch?: string;
        tip?: string;
        message?: string;
    }) => { branch: string; newTip: string; rewritten: number };
    "changelog-render": (options: {
        cwd?: string;
        version?: string;
        fromRef?: string;
        toRef?: string;
        write?: boolean;
        tags?: boolean;
        repo?: string;
        authorMap?: string;
        releasesDir?: string;
    }) => {
        notes: string;
        version: string;
        fromRef?: string;
        toRef: string;
        commitCount: number;
        writtenPath?: string;
        rangeLabel: string;
    };
    "changelog-lookup": (options: {
        cwd?: string;
        email?: string;
        login?: string;
        map?: string;
        githubToken?: string;
        fetch?: boolean;
    }) => GithubAuthor;
};

export type LintDiagnosticRecord = {
    rule: string;
    severity: "error" | "warning" | "info";
    message: string;
    subject?: string;
    hash?: string;
    path?: string;
    line?: number;
};

export type LintRunOptions = {
    cwd?: string;
    fromRef?: string;
    toRef?: string;
    subjects?: string[];
    rules?: Array<{ id: string; enabled?: boolean; severity?: "error" | "warning" | "info" }>;
    scanCargo?: boolean;
    commitOnly?: boolean;
};

export type LintReportRecord = {
    diagnostics: LintDiagnosticRecord[];
    errorCount: number;
    warningCount: number;
};

export type LintExports = {
    run: (options: LintRunOptions) => LintReportRecord;
    check: (options: LintRunOptions) => LintReportRecord;
};

export type UpdaterExports = {
    run: (options: { cwd?: string; interactive?: boolean }) => void;
};

export type FormatReportRecord = {
    formatted: number;
    unchanged: number;
    errors: string[];
};

export type FormatterStyleOptions = {
    indentStyle?: string;
    indentWidth?: number;
    lineWidth?: number;
    quoteStyle?: string;
};

export type FormatterRunOptions = {
    cwd?: string;
    check?: boolean;
    includes?: string[];
    excludes?: string[];
    rust?: boolean;
    javascript?: boolean;
    style?: FormatterStyleOptions;
};

export type FormatterExports = {
    run: (options: FormatterRunOptions) => FormatReportRecord;
};

export type PublisherExports = {
    "publish-workspace": (options: {
        cwd?: string;
        dryRun?: boolean;
        refresh?: boolean;
        placeholder?: boolean;
        only?: string;
        packages?: string[];
        version?: string;
        tag?: string;
        access?: string;
        npm?: string;
        otp?: string;
        totpSecret?: string;
        token?: string;
        trustRepo?: string;
        trustFile?: string;
        trustEnvironment?: string;
    }) => PublishReport;
    "trust-workspace": (options: {
        cwd?: string;
        dryRun?: boolean;
        refresh?: boolean;
        only?: string;
        packages?: string[];
        npm?: string;
        otp?: string;
        totpSecret?: string;
        token?: string;
        trustRepo?: string;
        trustFile?: string;
        trustEnvironment?: string;
    }) => TrustReport;
};

export type NiftyNative = {
    gitmoji: GitmojiExports;
    git: GitExports;
    history: HistoryExports;
    lint: LintExports;
    updater: UpdaterExports;
    formatter: FormatterExports;
    publisher: PublisherExports;
};

type NativeBinding = {
    gitmojiKnownGitmojis: () => string[];
    gitmojiValidateSubject: (subject: string) => boolean;
    gitmojiParseSubject: (subject: string) => ParsedSubject;
    gitmojiFormatSubject: (gitmoji: string, body: string) => string;
    gitmojiStripGitmoji: (subject: string) => string;
    gitmojiLeadingGitmoji: (subject: string) => string | null;
    gitmojiSectionForGitmoji: (gitmoji?: string | null) => string;
    gitmojiParseNoreplyEmail: (email: string) => GithubAuthor | null;
    gitmojiResolveGithubAuthor: (email: string, authorMapJson: string) => GithubAuthor | null;
    gitmojiDisplayLogin: (author: GithubAuthor) => string;
    gitmojiProfileUrl: (author: GithubAuthor) => string;
    gitmojiAvatarUrl: (author: GithubAuthor) => string;
    gitmojiAuthorMention: (email: string, authorName: string, authorMapJson: string) => string;
    gitmojiCommitBullet: (body: string, email: string, authorName: string, authorMapJson: string) => string;
    gitDiscoverRoot: (startPath: string) => string;
    gitListVersionTags: (repoRoot: string) => string[];
    gitListTagInfos: (repoRoot: string) => Array<{ name: string; shortHash: string }>;
    gitFormatTagList: (repoRoot: string) => string;
    gitResolveRange: (
        repoRoot: string,
        version?: string | null,
        fromRef?: string | null,
        toRef?: string | null,
    ) => { version: string; fromRef?: string; toRef: string };
    gitCollectCommits: (
        repoRoot: string,
        fromRef: string | null | undefined,
        toRef: string,
    ) => CommitRecord[];
    gitDetectGithubRepo: (repoRoot: string) => string | null;
    gitParseGithubRemote: (url: string) => string | null;
    publisherPublishWorkspace: (options: {
        cwd?: string | null;
        dryRun?: boolean | null;
        refresh?: boolean | null;
        placeholder?: boolean | null;
        tag?: string | null;
        access?: string | null;
        npm?: string | null;
        otp?: string | null;
        totpSecret?: string | null;
        token?: string | null;
        only?: string | null;
        packages?: string[] | null;
        version?: string | null;
    }) => PublishReport;
    publisherTrustWorkspace: (options: {
        cwd?: string | null;
        dryRun?: boolean | null;
        refresh?: boolean | null;
        only?: string | null;
        packages?: string[] | null;
        npm?: string | null;
        otp?: string | null;
        totpSecret?: string | null;
        token?: string | null;
    }) => TrustReport;
    gitToolsCommitExport: (options: {
        cwd?: string | null;
        base: string;
        ref?: string | null;
        path: string;
    }) => { count: number; path: string };
    gitToolsCommitApply: (options: {
        cwd?: string | null;
        base: string;
        ref?: string | null;
        path: string;
        dryRun?: boolean | null;
    }) => {
        changes: Array<{
            oldOid: string;
            oldSubject: string;
            newSubject: string;
            parentsRelinked: boolean;
            messageChanged: boolean;
        }>;
        oldTip: string;
        newTip?: string | null;
        refName: string;
        dryRun: boolean;
    };
    gitToolsRewordExport: (options: {
        cwd?: string | null;
        base: string;
        ref?: string | null;
        path: string;
    }) => { count: number; path: string };
    gitToolsRewordRewrite: (options: {
        cwd?: string | null;
        base: string;
        ref?: string | null;
        path: string;
        dryRun?: boolean | null;
    }) => {
        changes: Array<{
            oldOid: string;
            oldSubject: string;
            newSubject: string;
            parentsRelinked: boolean;
            messageChanged: boolean;
        }>;
        oldTip: string;
        newTip?: string | null;
        refName: string;
        dryRun: boolean;
    };
    gitToolsRetimeRange: (options: {
        cwd?: string | null;
        commit: string;
        startDate?: string | null;
        endDate?: string | null;
        branch?: string | null;
        tip?: string | null;
    }) => { branch: string; newTip: string; rewritten: number };
    gitToolsRetimeRoot: (options: {
        cwd?: string | null;
        startDate?: string | null;
        endDate?: string | null;
        branch?: string | null;
        tip?: string | null;
        message?: string | null;
    }) => { branch: string; newTip: string; rewritten: number };
    gitToolsChangelogRender: (options: {
        cwd?: string | null;
        version?: string | null;
        fromRef?: string | null;
        toRef?: string | null;
        write?: boolean | null;
        tags?: boolean | null;
        repo?: string | null;
        authorMap?: string | null;
        releasesDir?: string | null;
    }) => {
        notes: string;
        version: string;
        fromRef?: string | null;
        toRef: string;
        commitCount: number;
        writtenPath?: string | null;
        rangeLabel: string;
    };
    gitToolsChangelogLookup: (options: {
        cwd?: string | null;
        email?: string | null;
        login?: string | null;
        map?: string | null;
        githubToken?: string | null;
        fetch?: boolean | null;
    }) => GithubAuthor;
    lintRun: (options: {
        cwd?: string | null;
        fromRef?: string | null;
        toRef?: string | null;
        subjects?: string[] | null;
        rules?: Array<{ id: string; enabled?: boolean | null; severity?: string | null }> | null;
        scanCargo?: boolean | null;
        commitOnly?: boolean | null;
        check?: boolean | null;
    }) => {
        diagnostics: Array<{
            rule: string;
            severity: string;
            message: string;
            subject?: string | null;
            hash?: string | null;
            path?: string | null;
            line?: number | null;
        }>;
        errorCount: number;
        warningCount: number;
    };
    lintCheck: (options: {
        cwd?: string | null;
        fromRef?: string | null;
        toRef?: string | null;
        subjects?: string[] | null;
        rules?: Array<{ id: string; enabled?: boolean | null; severity?: string | null }> | null;
        scanCargo?: boolean | null;
        commitOnly?: boolean | null;
        check?: boolean | null;
    }) => {
        diagnostics: Array<{
            rule: string;
            severity: string;
            message: string;
            subject?: string | null;
            hash?: string | null;
            path?: string | null;
            line?: number | null;
        }>;
        errorCount: number;
        warningCount: number;
    };
    updaterRun: (options: { cwd?: string | null; interactive?: boolean | null }) => void;
    formatterRun: (options: {
        cwd?: string | null;
        check?: boolean | null;
        includes?: string[] | null;
        excludes?: string[] | null;
        rust?: boolean | null;
        javascript?: boolean | null;
        style?: FormatterStyleOptions | null;
    }) => {
        formatted: number;
        unchanged: number;
        errors: string[];
    };
};

const PLATFORM_PACKAGES: Record<string, string> = {
    "win32-x64": "@doki-land/nifty-win32-x64",
    "linux-x64": "@doki-land/nifty-linux-x64",
    "linux-arm64": "@doki-land/nifty-linux-arm64",
    "darwin-x64": "@doki-land/nifty-darwin-x64",
    "darwin-arm64": "@doki-land/nifty-darwin-arm64",
};

function mapAuthor(raw: GithubAuthor | null | undefined): GithubAuthor | undefined {
    if (!raw) {
        return undefined;
    }
    return {
        id: raw.id !== undefined && raw.id !== null ? BigInt(raw.id) : undefined,
        login: raw.login ?? undefined,
    };
}

function mapLintOptions(options: LintRunOptions) {
    return {
        ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
        ...(options.fromRef !== undefined ? { fromRef: options.fromRef } : {}),
        ...(options.toRef !== undefined ? { toRef: options.toRef } : {}),
        ...(options.subjects !== undefined ? { subjects: options.subjects } : {}),
        ...(options.rules !== undefined ? { rules: options.rules } : {}),
        ...(options.scanCargo !== undefined ? { scanCargo: options.scanCargo } : {}),
        ...(options.commitOnly !== undefined ? { commitOnly: options.commitOnly } : {}),
    };
}

function mapLintReport(raw: {
    diagnostics: Array<{
        rule: string;
        severity: string;
        message: string;
        subject?: string | null;
        hash?: string | null;
        path?: string | null;
        line?: number | null;
    }>;
    errorCount: number;
    warningCount: number;
}): LintReportRecord {
    return {
        errorCount: raw.errorCount,
        warningCount: raw.warningCount,
        diagnostics: raw.diagnostics.map((item) => ({
            rule: item.rule,
            severity: item.severity as LintDiagnosticRecord["severity"],
            message: item.message,
            subject: item.subject ?? undefined,
            hash: item.hash ?? undefined,
            path: item.path ?? undefined,
            line: item.line ?? undefined,
        })),
    };
}

function wrapBinding(binding: NativeBinding): NiftyNative {
    return {
        gitmoji: {
            "known-gitmojis": () => binding.gitmojiKnownGitmojis(),
            "validate-subject": (subject) => binding.gitmojiValidateSubject(subject),
            "parse-subject": (subject) => binding.gitmojiParseSubject(subject),
            "format-subject": (gitmoji, body) => binding.gitmojiFormatSubject(gitmoji, body),
            "strip-gitmoji": (subject) => binding.gitmojiStripGitmoji(subject),
            "leading-gitmoji": (subject) => binding.gitmojiLeadingGitmoji(subject) ?? undefined,
            "section-for-gitmoji": (gitmoji) => binding.gitmojiSectionForGitmoji(gitmoji) as ReleaseSection,
            "parse-noreply-email": (email) => mapAuthor(binding.gitmojiParseNoreplyEmail(email)),
            "resolve-github-author": (email, authorMapJson) =>
                mapAuthor(binding.gitmojiResolveGithubAuthor(email, authorMapJson)),
            "display-login": (author) => binding.gitmojiDisplayLogin(author),
            "profile-url": (author) => binding.gitmojiProfileUrl(author),
            "avatar-url": (author) => binding.gitmojiAvatarUrl(author),
            "author-mention": (email, authorName, authorMapJson) =>
                binding.gitmojiAuthorMention(email, authorName, authorMapJson),
            "commit-bullet": (body, email, authorName, authorMapJson) =>
                binding.gitmojiCommitBullet(body, email, authorName, authorMapJson),
        },
        git: {
            "discover-root": (startPath) => binding.gitDiscoverRoot(startPath),
            "list-version-tags": (repoRoot) => binding.gitListVersionTags(repoRoot),
            "list-tag-infos": (repoRoot) =>
                binding.gitListTagInfos(repoRoot).map((tag) => ({
                    name: tag.name,
                    "short-hash": tag.shortHash,
                })),
            "format-tag-list": (repoRoot) => binding.gitFormatTagList(repoRoot),
            "resolve-range": (repoRoot, version, fromRef, toRef) => {
                const range = binding.gitResolveRange(repoRoot, version, fromRef, toRef);
                return {
                    version: range.version,
                    "from-ref": range.fromRef,
                    "to-ref": range.toRef,
                };
            },
            "collect-commits": (repoRoot, fromRef, toRef) => binding.gitCollectCommits(repoRoot, fromRef, toRef),
            "detect-github-repo": (repoRoot) => binding.gitDetectGithubRepo(repoRoot) ?? undefined,
            "parse-github-remote": (url) => binding.gitParseGithubRemote(url) ?? undefined,
        },
        history: {
            "commit-export": (options) =>
                binding.gitToolsCommitExport({
                    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
                    base: options.base,
                    ...(options.ref !== undefined ? { ref: options.ref } : {}),
                    path: options.path,
                }),
            "commit-apply": (options) => {
                const report = binding.gitToolsCommitApply({
                    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
                    base: options.base,
                    ...(options.ref !== undefined ? { ref: options.ref } : {}),
                    path: options.path,
                    ...(options.dryRun !== undefined ? { dryRun: options.dryRun } : {}),
                });
                return {
                    changes: report.changes,
                    oldTip: report.oldTip,
                    newTip: report.newTip ?? undefined,
                    refName: report.refName,
                    dryRun: report.dryRun,
                };
            },
            "reword-export": (options) =>
                binding.gitToolsRewordExport({
                    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
                    base: options.base,
                    ...(options.ref !== undefined ? { ref: options.ref } : {}),
                    path: options.path,
                }),
            "reword-rewrite": (options) => {
                const report = binding.gitToolsRewordRewrite({
                    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
                    base: options.base,
                    ...(options.ref !== undefined ? { ref: options.ref } : {}),
                    path: options.path,
                    ...(options.dryRun !== undefined ? { dryRun: options.dryRun } : {}),
                });
                return {
                    changes: report.changes,
                    oldTip: report.oldTip,
                    newTip: report.newTip ?? undefined,
                    refName: report.refName,
                    dryRun: report.dryRun,
                };
            },
            "retime-range": (options) =>
                binding.gitToolsRetimeRange({
                    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
                    commit: options.commit,
                    ...(options.startDate !== undefined ? { startDate: options.startDate } : {}),
                    ...(options.endDate !== undefined ? { endDate: options.endDate } : {}),
                    ...(options.branch !== undefined ? { branch: options.branch } : {}),
                    ...(options.tip !== undefined ? { tip: options.tip } : {}),
                }),
            "retime-root": (options) =>
                binding.gitToolsRetimeRoot({
                    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
                    ...(options.startDate !== undefined ? { startDate: options.startDate } : {}),
                    ...(options.endDate !== undefined ? { endDate: options.endDate } : {}),
                    ...(options.branch !== undefined ? { branch: options.branch } : {}),
                    ...(options.tip !== undefined ? { tip: options.tip } : {}),
                    ...(options.message !== undefined ? { message: options.message } : {}),
                }),
            "changelog-render": (options) => {
                const report = binding.gitToolsChangelogRender({
                    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
                    ...(options.version !== undefined ? { version: options.version } : {}),
                    ...(options.fromRef !== undefined ? { fromRef: options.fromRef } : {}),
                    ...(options.toRef !== undefined ? { toRef: options.toRef } : {}),
                    ...(options.write !== undefined ? { write: options.write } : {}),
                    ...(options.tags !== undefined ? { tags: options.tags } : {}),
                    ...(options.repo !== undefined ? { repo: options.repo } : {}),
                    ...(options.authorMap !== undefined ? { authorMap: options.authorMap } : {}),
                    ...(options.releasesDir !== undefined ? { releasesDir: options.releasesDir } : {}),
                });
                return {
                    notes: report.notes,
                    version: report.version,
                    fromRef: report.fromRef ?? undefined,
                    toRef: report.toRef,
                    commitCount: report.commitCount,
                    writtenPath: report.writtenPath ?? undefined,
                    rangeLabel: report.rangeLabel,
                };
            },
            "changelog-lookup": (options) => {
                const author = binding.gitToolsChangelogLookup({
                    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
                    ...(options.email !== undefined ? { email: options.email } : {}),
                    ...(options.login !== undefined ? { login: options.login } : {}),
                    ...(options.map !== undefined ? { map: options.map } : {}),
                    ...(options.githubToken !== undefined ? { githubToken: options.githubToken } : {}),
                    ...(options.fetch !== undefined ? { fetch: options.fetch } : {}),
                });
                return mapAuthor(author) ?? {};
            },
        },
        lint: {
            run: (options) => mapLintReport(binding.lintRun(mapLintOptions(options))),
            check: (options) => mapLintReport(binding.lintCheck(mapLintOptions(options))),
        },
        updater: {
            run: (options) => {
                binding.updaterRun({
                    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
                    ...(options.interactive !== undefined ? { interactive: options.interactive } : {}),
                });
            },
        },
        formatter: {
            run: (options) =>
                binding.formatterRun({
                    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
                    ...(options.check !== undefined ? { check: options.check } : {}),
                    ...(options.includes !== undefined ? { includes: options.includes } : {}),
                    ...(options.excludes !== undefined ? { excludes: options.excludes } : {}),
                    ...(options.rust !== undefined ? { rust: options.rust } : {}),
                    ...(options.javascript !== undefined ? { javascript: options.javascript } : {}),
                    ...(options.style !== undefined ? { style: options.style } : {}),
                }),
        },
        publisher: {
            "publish-workspace": (options) =>
                binding.publisherPublishWorkspace({
                    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
                    ...(options.dryRun !== undefined ? { dryRun: options.dryRun } : {}),
                    ...(options.refresh !== undefined ? { refresh: options.refresh } : {}),
                    ...(options.placeholder !== undefined ? { placeholder: options.placeholder } : {}),
                    ...(options.only !== undefined ? { only: options.only } : {}),
                    ...(options.packages !== undefined ? { packages: options.packages } : {}),
                    ...(options.version !== undefined ? { version: options.version } : {}),
                    ...(options.tag !== undefined ? { tag: options.tag } : {}),
                    ...(options.access !== undefined ? { access: options.access } : {}),
                    ...(options.npm !== undefined ? { npm: options.npm } : {}),
                    ...(options.otp !== undefined ? { otp: options.otp } : {}),
                    ...(options.totpSecret !== undefined ? { totpSecret: options.totpSecret } : {}),
                    ...(options.token !== undefined ? { token: options.token } : {}),
                    ...(options.trustRepo !== undefined ? { trustRepo: options.trustRepo } : {}),
                    ...(options.trustFile !== undefined ? { trustFile: options.trustFile } : {}),
                    ...(options.trustEnvironment !== undefined
                        ? { trustEnvironment: options.trustEnvironment }
                        : {}),
                }),
            "trust-workspace": (options) =>
                binding.publisherTrustWorkspace({
                    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
                    ...(options.dryRun !== undefined ? { dryRun: options.dryRun } : {}),
                    ...(options.refresh !== undefined ? { refresh: options.refresh } : {}),
                    ...(options.only !== undefined ? { only: options.only } : {}),
                    ...(options.packages !== undefined ? { packages: options.packages } : {}),
                    ...(options.npm !== undefined ? { npm: options.npm } : {}),
                    ...(options.otp !== undefined ? { otp: options.otp } : {}),
                    ...(options.totpSecret !== undefined ? { totpSecret: options.totpSecret } : {}),
                    ...(options.token !== undefined ? { token: options.token } : {}),
                    ...(options.trustRepo !== undefined ? { trustRepo: options.trustRepo } : {}),
                    ...(options.trustFile !== undefined ? { trustFile: options.trustFile } : {}),
                    ...(options.trustEnvironment !== undefined
                        ? { trustEnvironment: options.trustEnvironment }
                        : {}),
                }),
        },
    };
}

let cached: NiftyNative | undefined;

/** Load the platform-specific Node-API binary from `@doki-land/nifty-<platform>`. */
export function loadNiftyNative(): NiftyNative {
    if (cached) {
        return cached;
    }
    const key = `${process.platform}-${process.arch}`;
    const pkg = PLATFORM_PACKAGES[key];
    if (!pkg) {
        throw new Error(`Unsupported platform for Nifty native bindings: ${key}`);
    }
    const require = createRequire(import.meta.url);
    const binding = require(pkg).default as NativeBinding;
    cached = wrapBinding(binding);
    return cached;
}

export function mapTagInfo(raw: { name: string; "short-hash": string }): TagInfo {
    return { name: raw.name, shortHash: raw["short-hash"] };
}

export function mapRangeInfo(raw: { version: string; "from-ref"?: string; "to-ref": string }): RangeInfo {
    return {
        version: raw.version,
        fromRef: raw["from-ref"],
        toRef: raw["to-ref"],
    };
}

export function mapCommitRecord(raw: NativeCommitRecord): CommitRecord {
    return {
        hash: raw.hash,
        email: raw.email,
        author: raw.author,
        subject: raw.subject,
        body: raw.body,
        gitmoji: raw.gitmoji,
        section: raw.section as ReleaseSection,
    };
}
