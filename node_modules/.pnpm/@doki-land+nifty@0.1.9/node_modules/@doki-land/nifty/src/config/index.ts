export { defineConfig } from "./defineConfig.js";
export {
    detectProjectLayout,
    discoverPackageDirs,
    findCargoManifest,
    findPackageManifest,
    type ProjectKind,
    type ProjectLayout,
} from "./detectProject.js";
export {
    DEFAULT_AUTHOR_MAP_PATH,
    loadAuthorMapJson,
    resolveAuthorMapPath,
} from "./authorMap.js";
export {
    defaultConfigPath,
    ensureConfigFile,
    findConfigFile,
    loadConfig,
    type LoadConfigOptions,
    type LoadedNiftyConfig,
} from "./loadConfig.js";
export { resolveFormatConfig, type ResolvedFormatOptions } from "./formatPresets.js";
export { trustPayloadFromConfig } from "./trustPayload.js";
export {
    CONFIG_FILE_NAMES,
    type NiftyAuthorEntry,
    type NiftyConfig,
    type NiftyConfigEnv,
    type NiftyConfigExport,
    type NiftyFormatConfig,
    type NiftyFormatPreset,
    type NiftyFormatStyleConfig,
    type NiftyTrustConfig,
    type NiftyTrustNpmConfig,
} from "./types.js";
