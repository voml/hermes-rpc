import { existsSync, readFileSync } from "node:fs";
import { isAbsolute, join } from "node:path";

import type { NiftyConfig } from "./types.js";

/** Default path relative to repo root (email → GitHub author JSON). */
export const DEFAULT_AUTHOR_MAP_PATH = "documentation/maintenance/author-github.json";

/** Resolve configured author map path to an absolute filesystem path. */
export function resolveAuthorMapPath(config: NiftyConfig, repoRoot: string): string {
    const configured = config.authorMap ?? DEFAULT_AUTHOR_MAP_PATH;
    return isAbsolute(configured) ? configured : join(repoRoot, configured);
}

/** Load author map file contents for native helpers expecting JSON text. */
export function loadAuthorMapJson(repoRoot: string, authorMapPath?: string): string {
    const abs = authorMapPath
        ? isAbsolute(authorMapPath)
            ? authorMapPath
            : join(repoRoot, authorMapPath)
        : resolveAuthorMapPath({}, repoRoot);
    if (!existsSync(abs)) {
        return "{}";
    }
    return readFileSync(abs, "utf8");
}
