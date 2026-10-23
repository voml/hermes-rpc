import { loadConfig, resolveFormatConfig } from "../config/index.js";
import { loadNiftyNative } from "../native.js";

export type FormatOptions = {
    check?: boolean;
    cwd?: string;
};

export async function formatWorkspace(options: FormatOptions): Promise<number> {
    const cwd = options.cwd ?? process.cwd();
    const { config } = await loadConfig({ cwd, createIfMissing: false, env: { command: "format" } });
    const resolved = resolveFormatConfig(config.format);
    const native = loadNiftyNative();
    const report = native.formatter.run({
        cwd,
        check: options.check,
        includes: resolved.includes,
        excludes: resolved.excludes,
        rust: resolved.rust,
        javascript: resolved.javascript,
        style: resolved.style,
    });

    if (report.errors.length > 0) {
        for (const message of report.errors) {
            console.error(`error: ${message}`);
        }
    }

    if (report.formatted === 0 && report.unchanged > 0 && report.errors.length === 0) {
        console.log(`format: no changes needed (preset ${resolved.preset})`);
    } else if (report.errors.length === 0) {
        console.log(
            `format: ${report.formatted} file(s) updated, ${report.unchanged} unchanged (preset ${resolved.preset})`,
        );
    }

    return report.errors.length > 0 ? 1 : 0;
}
