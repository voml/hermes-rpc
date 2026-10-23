import { loadNiftyNative } from "../native.js";

export type UpdateOptions = {
    interactive: boolean;
    cwd?: string;
};

export async function runUpdate(argv: string[]): Promise<void> {
    await updateWorkspace(parseUpdateArgs(argv));
}

export async function updateWorkspace(options: UpdateOptions): Promise<void> {
    const native = loadNiftyNative();
    native.updater.run({
        cwd: options.cwd,
        interactive: options.interactive,
    });
}

function parseUpdateArgs(argv: string[]): UpdateOptions {
    let interactive = false;
    let cwd: string | undefined;
    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === "-i" || arg === "--interactive") {
            interactive = true;
        } else if (arg === "-C" || arg === "--cwd") {
            cwd = argv[++i];
        }
    }
    return { interactive, cwd };
}
