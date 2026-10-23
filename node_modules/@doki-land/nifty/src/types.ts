/** GitHub author resolved from noreply email or `author-github.json`. */
export type GithubAuthor = {
    id?: bigint;
    login?: string;
};

/** Nifty gitmoji-parsed commit subject. */
export type ParsedSubject = {
    gitmoji?: string;
    body: string;
    /** `features` | `fixes` | `breaking` | `other` */
    section: string;
};

/** Release note section keys (Nifty gitmoji grouping). */
export type ReleaseSection = "features" | "fixes" | "breaking" | "other";

/** Semver tag with short hash. */
export type TagInfo = {
    name: string;
    shortHash: string;
};

/** Resolved release tag range. */
export type RangeInfo = {
    version: string;
    fromRef?: string;
    toRef: string;
};

/** Commit from repository history with Nifty gitmoji fields. */
export type CommitRecord = {
    hash: string;
    email: string;
    author: string;
    subject: string;
    body: string;
    gitmoji?: string;
    section: ReleaseSection;
};
