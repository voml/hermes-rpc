import { resolve } from "node:path";

import type { ParsedOptions } from "@vmz/commander";

import { resolveNpmExecutable } from "./resolveNpm.js";
import type { AuthCliOptions } from "./authArgs.js";

export function cwdFrom(options: ParsedOptions): string | undefined {
    const value = options.cwd;
    return typeof value === "string" && value.length > 0 ? resolve(value) : undefined;
}

export function flag(options: ParsedOptions, key: string): boolean {
    return options[key] === true;
}

export function str(options: ParsedOptions, key: string): string | undefined {
    const value = options[key];
    return typeof value === "string" && value.length > 0 ? value : undefined;
}

export function strList(options: ParsedOptions, key: string): string[] {
    const value = options[key];
    if (Array.isArray(value)) {
        return value.filter((entry): entry is string => typeof entry === "string" && entry.length > 0);
    }
    if (typeof value === "string" && value.length > 0) {
        return [value];
    }
    return [];
}

export function authFromOptions(options: ParsedOptions): AuthCliOptions {
    return {
        otp: str(options, "otp"),
        totpSecret: str(options, "totp-secret"),
        token: str(options, "npm-token") ?? str(options, "token"),
        npm: resolveNpmExecutable(),
    };
}
