import type { NiftyConfig, NiftyConfigEnv, NiftyConfigExport } from "./types.js";

type NiftyConfigFactory = (env: NiftyConfigEnv) => NiftyConfig | Promise<NiftyConfig>;

type DefineConfigInput<T> = T extends NiftyConfigFactory
    ? T
    : T extends NiftyConfig
      ? T & Record<Exclude<keyof T, keyof NiftyConfig>, never>
      : never;

/** Type-safe helper for `nifty.config.ts` / `nifty.config.js`. */
export function defineConfig<T extends NiftyConfigExport>(config: DefineConfigInput<T>): T {
    return config;
}
