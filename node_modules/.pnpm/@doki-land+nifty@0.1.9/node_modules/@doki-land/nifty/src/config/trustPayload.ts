import type { NiftyConfig } from "./types.js";

/** Map `nifty.config` `trust.npm` into native publisher options. */
export function trustPayloadFromConfig(config: NiftyConfig) {
    const npm = config.trust?.npm;
    if (!npm) {
        return {};
    }
    return {
        ...(npm.repo ? { trustRepo: npm.repo } : {}),
        ...(npm.file ? { trustFile: npm.file } : {}),
        ...(npm.environment ? { trustEnvironment: npm.environment } : {}),
    };
}
