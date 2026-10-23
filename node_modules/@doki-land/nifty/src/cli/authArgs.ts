import { resolveNpmExecutable } from "./resolveNpm.js";

export type AuthCliOptions = {
    otp?: string;
    totpSecret?: string;
    token?: string;
    npm: string;
};

export function parseAuthArgs(argv: string[]): AuthCliOptions {
    let otp: string | undefined;
    let totpSecret: string | undefined;
    let token: string | undefined;

    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === "--otp") {
            otp = argv[++i];
        } else if (arg === "--totp-secret") {
            totpSecret = argv[++i];
        } else if (arg === "--token") {
            token = argv[++i];
        } else if (arg.startsWith("--otp=")) {
            otp = arg.slice("--otp=".length);
        } else if (arg.startsWith("--totp-secret=")) {
            totpSecret = arg.slice("--totp-secret=".length);
        } else if (arg.startsWith("--token=")) {
            token = arg.slice("--token=".length);
        }
    }

    return {
        otp,
        totpSecret,
        token,
        npm: resolveNpmExecutable(),
    };
}

export function authPayload(options: AuthCliOptions): {
    otp?: string;
    totpSecret?: string;
    token?: string;
    npm: string;
} {
    return {
        ...(options.otp !== undefined ? { otp: options.otp } : {}),
        ...(options.totpSecret !== undefined ? { totpSecret: options.totpSecret } : {}),
        ...(options.token !== undefined ? { token: options.token } : {}),
        npm: options.npm,
    };
}
