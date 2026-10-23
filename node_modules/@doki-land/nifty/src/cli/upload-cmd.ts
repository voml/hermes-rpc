import type { Cli, ParsedOptions } from "@vmz/commander";

import { bootstrapFromOptions } from "./context.js";
import { uploadFromOptions } from "./upload.js";
import { cwdFrom, flag, str } from "./options.js";

export function registerUploadCommand(cli: Cli): void {
    cli.command("upload", "cli.cmd.upload")
        .option("--release", "cli.opt.release")
        .option("--pages", "cli.opt.pages")
        .option("--both", "cli.opt.both")
        .option("-d, --dir <path>", "cli.opt.dir")
        .option("-t, --tag <tag>", "cli.opt.tag")
        .option("--repo <repo>", "cli.opt.repo")
        .option("--token <token>", "cli.opt.token")
        .option("--name <title>", "cli.opt.name")
        .option("--notes <body>", "cli.opt.notes")
        .option("--notes-file <path>", "cli.opt.notes-file")
        .option("--draft", "cli.opt.draft")
        .option("--no-generate-notes", "cli.opt.no-generate-notes")
        .option("--github-action", "cli.opt.github-action")
        .action(async (options) => cmdUpload(options));
}

export async function cmdUpload(options: ParsedOptions): Promise<number> {
    await bootstrapFromOptions(options);
    await uploadFromOptions({
        release: flag(options, "release"),
        pages: flag(options, "pages"),
        both: flag(options, "both"),
        dir: str(options, "dir") ?? "dist",
        tag: str(options, "tag"),
        repo: str(options, "repo"),
        token: str(options, "token"),
        name: str(options, "name"),
        notes: str(options, "notes"),
        notesFile: str(options, "notes-file"),
        draft: flag(options, "draft"),
        generateNotes: !flag(options, "no-generate-notes"),
        githubAction: flag(options, "github-action"),
        cwd: cwdFrom(options),
    });
    return 0;
}
