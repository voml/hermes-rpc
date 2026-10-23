/**
 * @doki-land/nifty — gitmoji-first commit conventions and gix-backed git helpers.
 *
 * Build native bindings first: `npm run build:napi` at the repo root.
 * CLI (`nifty update`, `nifty lint`, `nifty upload`) is provided via the `nifty` bin.
 */

export type {
    CommitRecord,
    GithubAuthor,
    ParsedSubject,
    RangeInfo,
    ReleaseSection,
    TagInfo,
} from "./types.js";
export { Git, createGit, type ResolveRangeOptions } from "./git.js";
export { Github, createGithub, type LookupUserOptions } from "./github.js";
export { Gitmoji, createGitmoji } from "./gitmoji.js";
export { Nifty, createNifty, type NiftyOpenOptions } from "./nifty.js";
export {
    defineConfig,
    loadConfig,
    findConfigFile,
    DEFAULT_AUTHOR_MAP_PATH,
    loadAuthorMapJson,
    resolveAuthorMapPath,
    detectProjectLayout,
    discoverPackageDirs,
    findCargoManifest,
    findPackageManifest,
    type LoadConfigOptions,
    type LoadedNiftyConfig,
    resolveFormatConfig,
    type NiftyAuthorEntry,
    type NiftyConfig,
    type NiftyConfigEnv,
    type NiftyFormatConfig,
    type NiftyFormatPreset,
    type NiftyFormatStyleConfig,
    type NiftyTrustConfig,
    type NiftyTrustNpmConfig,
    trustPayloadFromConfig,
    type ProjectKind,
    type ProjectLayout,
    type ResolvedFormatOptions,
} from "./config/index.js";
