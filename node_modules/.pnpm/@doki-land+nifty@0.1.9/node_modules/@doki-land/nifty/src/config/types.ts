/** One email → GitHub author row in `author-github.json`. */
export type NiftyAuthorEntry = {
    id?: number;
    login?: string;
};

/** Built-in workspace format presets (`nifty.config` `format.preset`). */
export type NiftyFormatPreset = "default" | "nifty" | "npm-tools" | "vmz";

/** Oak formatter style knobs (`nifty.config` `format.style`). */
export type NiftyFormatStyleConfig = {
    /** Indent with spaces or tabs (default: `space`). */
    indentStyle?: "space" | "tab";
    /** Spaces or tabs per indent level (default: `4`). */
    indentWidth?: number;
    /** Soft line width (default: `144`). */
    lineWidth?: number;
    /** String quote preference (default: `single`). */
    quoteStyle?: "single" | "double";
};

/** npm Trusted Publisher target for `nifty trust` (GitHub Actions OIDC). */
export type NiftyTrustNpmConfig = {
    /** GitHub `owner/repo` for the publishing workflow. */
    repo?: string;
    /** Workflow filename, e.g. `release-npm.yml`. */
    file?: string;
    /** GitHub Environment name, e.g. `NPM_PUBLISH`. */
    environment?: string;
};

/** Trusted publisher wiring (`nifty trust`). Only `npm` is supported today. */
export type NiftyTrustConfig = {
    npm?: NiftyTrustNpmConfig;
};

/** Workspace format options (`nifty format` reads `nifty.config` `format`). */
export type NiftyFormatConfig = {
    /** Select built-in include roots for Nifty hybrid monorepos. */
    preset?: NiftyFormatPreset;
    /** Extra include globs (override preset list when set alone). */
    includes?: string[];
    /** Skip paths under these globs after includes expand. */
    excludes?: string[];
    /** Run `cargo fmt` (default: true when a Cargo workspace is detected). */
    rust?: boolean;
    /** Run Oak formatting on JS/TS targets (default: true). */
    javascript?: boolean;
    /** Inline Oak formatter style (not an external config file path). */
    style?: NiftyFormatStyleConfig;
};

/** Nifty project configuration (`nifty.config.ts` / `nifty.config.js`). */
export type NiftyConfig = {
    /** GitHub personal access token for email search / profile fetch. */
    githubToken?: string;
    /** Path to `author-github.json` (default: `documentation/maintenance/author-github.json`). */
    authorMap?: string;
    /** Repository root override when discovering git metadata. */
    repoRoot?: string;
    /** Cargo workspace / crate root override. */
    cargoRoot?: string;
    /** npm package root override. */
    npmRoot?: string;
    /** npm publish / Trusted Publisher (CI uses publish-npm.yml). */
    publish?: {
        /**
         * Optional narrow subset for `nifty publish` / `nifty trust`.
         * Default: every workspace package that is not `private: true` (and not blocked by a private dep).
         */
        packages?: string[];
    };
    /**
     * Trusted publisher wiring for `nifty trust`.
     * Prefer `trust.npm` over `NIFTY_TRUST_*` env vars. `nifty trust` uses the npm registry API.
     */
    trust?: NiftyTrustConfig;
    /** Release reference changelog defaults (`nifty change-logs`). */
    changelog?: {
        /** GitHub `owner/repo` for contrib.rocks (default: parse `origin`). */
        repo?: string;
        /** `--write` output directory (default: `documentation/maintenance/releases`). */
        releasesDir?: string;
    };
    /** Rule-based lint configuration. */
    lint?: {
        rules?: Array<{
            id: string;
            enabled?: boolean;
            severity?: "error" | "warning" | "info";
        }>;
    };
    /** Workspace format targets and engines (`nifty format`). */
    format?: NiftyFormatConfig;
};

export type NiftyConfigExport = NiftyConfig | ((env: NiftyConfigEnv) => NiftyConfig | Promise<NiftyConfig>);

export type NiftyConfigEnv = {
    mode: string;
    command: string;
    /** Detected cargo + npm project layout for hybrid monorepos. */
    layout: import("./detectProject.js").ProjectLayout;
};

export const CONFIG_FILE_NAMES = [
    "nifty.config.ts",
    "nifty.config.js",
    "nifty.config.mjs",
    "nifty.config.cjs",
] as const;
