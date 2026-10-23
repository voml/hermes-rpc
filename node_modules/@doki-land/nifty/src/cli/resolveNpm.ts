import { existsSync } from "node:fs";
import { dirname, join } from "node:path";

/** Resolve npm next to the active Node binary (Windows needs `npm.cmd`). */
export function resolveNpmExecutable(): string {
    if (process.env.NIFTY_NPM) {
        return process.env.NIFTY_NPM;
    }

    const sibling = join(dirname(process.execPath), process.platform === "win32" ? "npm.cmd" : "npm");
    if (existsSync(sibling)) {
        return sibling;
    }

    return process.platform === "win32" ? "npm.cmd" : "npm";
}
