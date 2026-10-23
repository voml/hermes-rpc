import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, relative, resolve } from "node:path";

import { detectProjectLayout } from "../config/detectProject.js";
import { loadNiftyNative } from "../native.js";

const USER_AGENT = "nifty-upload";

export type UploadOptions = {
    release: boolean;
    pages: boolean;
    both: boolean;
    dir: string;
    tag?: string;
    repo?: string;
    token?: string;
    name?: string;
    notes?: string;
    notesFile?: string;
    draft: boolean;
    generateNotes: boolean;
    githubAction: boolean;
    cwd?: string;
};

type GithubRelease = {
    id: number;
};

export async function runUpload(argv: string[]): Promise<void> {
    await uploadFromOptions(parseUploadArgs(argv));
}

export async function uploadFromOptions(options: UploadOptions): Promise<void> {
    if (options.githubAction) {
        applyGithubActionDefaults(options);
    }

    const token = options.token ?? process.env.GITHUB_TOKEN;
    if (!token) {
        throw new Error("GitHub token is required (set --token or GITHUB_TOKEN)");
    }

    const cwd = options.cwd ?? process.cwd();
    const layout = detectProjectLayout(cwd);
    const native = loadNiftyNative();
    const repo =
        options.repo ??
        native.git["detect-github-repo"](layout.root) ??
        process.env.GITHUB_REPOSITORY;
    if (!repo) {
        throw new Error("could not detect GitHub repo (use --repo owner/name)");
    }

    const tag = options.tag ?? process.env.GITHUB_REF_NAME;
    if (!tag) {
        throw new Error("release tag is required (use --tag or push a git tag)");
    }

    const target = parseTarget(options);
    const notes = await resolveNotes(options, layout.root, tag, native);

    if (target === "release" || target === "both") {
        await uploadReleaseAssets(repo, token, tag, options, notes);
    }
    if (target === "pages" || target === "both") {
        await deployGithubPages(repo, token, options.dir);
    }
}

function parseUploadArgs(argv: string[]): UploadOptions {
    const options: UploadOptions = {
        release: false,
        pages: false,
        both: false,
        dir: "dist",
        draft: false,
        generateNotes: true,
        githubAction: false,
    };

    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === "--release") options.release = true;
        else if (arg === "--pages") options.pages = true;
        else if (arg === "--both") options.both = true;
        else if (arg === "-d" || arg === "--dir") options.dir = argv[++i];
        else if (arg === "-h" || arg === "--help") {
            printUploadHelp();
            process.exit(0);
        }
        else if (arg === "-t" || arg === "--tag") options.tag = argv[++i];
        else if (arg === "--repo") options.repo = argv[++i];
        else if (arg === "--token") options.token = argv[++i];
        else if (arg === "--name") options.name = argv[++i];
        else if (arg === "--notes") options.notes = argv[++i];
        else if (arg === "--notes-file") options.notesFile = argv[++i];
        else if (arg === "--draft") options.draft = true;
        else if (arg === "--no-generate-notes") options.generateNotes = false;
        else if (arg === "--github-action") options.githubAction = true;
        else if (arg === "-C" || arg === "--cwd") options.cwd = argv[++i];
    }

    return options;
}

function parseTarget(options: UploadOptions): "release" | "pages" | "both" {
    if (options.both) return "both";
    if (options.pages) return "pages";
    if (options.release) return "release";
    throw new Error("specify --release, --pages, or --both");
}

function applyGithubActionDefaults(options: UploadOptions): void {
    options.token ??= process.env.GITHUB_TOKEN;
    options.repo ??= process.env.GITHUB_REPOSITORY;
    options.tag ??= process.env.GITHUB_REF_NAME;
    options.cwd ??= process.env.GITHUB_WORKSPACE;
}

async function resolveNotes(
    options: UploadOptions,
    repoRoot: string,
    tag: string,
    native: ReturnType<typeof loadNiftyNative>,
): Promise<string> {
    if (options.notes) return options.notes;
    if (options.notesFile) return readFileSync(options.notesFile, "utf8");
    if (!options.generateNotes) return "";
    try {
        const range = native.git["resolve-range"](repoRoot, tag.replace(/^v/, ""), undefined, tag);
        const commits = native.git["collect-commits"](repoRoot, range["from-ref"], range["to-ref"]);
        return commits.map((commit) => `- ${commit.body}`).join("\n");
    } catch {
        return "";
    }
}

async function uploadReleaseAssets(
    repo: string,
    token: string,
    tag: string,
    options: UploadOptions,
    notes: string,
): Promise<void> {
    const [owner, name] = splitRepo(repo);
    let release = await apiReleaseByTag(owner, name, tag, token);
    if (!release) {
        release = await apiCreateRelease(owner, name, tag, options.name ?? tag, notes, options.draft, token);
    }
    const cwd = options.cwd ?? process.cwd();
    const files = collectFiles(resolve(cwd, options.dir));
    for (const file of files) {
        const asset =
            relative(resolve(cwd, options.dir), file).split(/[/\\]/).join("-") || basename(file);
        await apiUploadAsset(owner, name, release.id, asset, file, token);
        console.log(`release: uploaded ${asset}`);
    }
}

function printUploadHelp(): void {
    console.log(`nifty upload — GitHub Release assets and/or GitHub Pages

Usage:
  nifty upload --release [--dir dist] [--tag vX.Y.Z]
  nifty upload --pages --dir dist
  nifty upload --both --dir dist
  nifty upload --release --github-action

Options:
  --release         upload files from --dir to the GitHub Release for --tag
  --pages           push --dir to gh-pages
  --both            release + pages
  -d, --dir <path>  directory tree for --release/--pages (default: dist)
  --github-action   read GITHUB_TOKEN, GITHUB_REPOSITORY, GITHUB_REF_NAME
`);
}

async function deployGithubPages(repo: string, token: string, dir: string): Promise<void> {
    const [owner, name] = splitRepo(repo);
    const source = resolve(dir);
    const work = mkdtempSync(join(tmpdir(), "nifty-pages-"));
    cpSync(source, work, { recursive: true });
    const nojekyll = join(work, ".nojekyll");
    if (!existsSync(nojekyll)) {
        writeFileSync(nojekyll, "");
    }
    git(work, ["init"]);
    git(work, ["config", "user.name", "github-actions[bot]"]);
    git(work, ["config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"]);
    git(work, ["add", "."]);
    git(work, ["commit", "-m", "Deploy GitHub Pages"]);
    const remote = `https://x-access-token:${token}@github.com/${owner}/${name}.git`;
    git(work, ["remote", "add", "origin", remote]);
    git(work, ["push", "--force", "origin", "HEAD:gh-pages"]);
    console.log(`pages: pushed ${source} to gh-pages`);
}

function collectFiles(dir: string): string[] {
    const out: string[] = [];
    for (const entry of walk(dir)) {
        if (statSync(entry).isFile()) {
            out.push(entry);
        }
    }
    return out;
}

function walk(dir: string): string[] {
    const files: string[] = [];
    for (const entry of readdirSync(dir)) {
        const path = join(dir, entry);
        if (statSync(path).isDirectory()) {
            files.push(...walk(path));
        } else {
            files.push(path);
        }
    }
    return files;
}

function splitRepo(repo: string): [string, string] {
    const parts = repo.split("/");
    if (parts.length !== 2 || !parts[0] || !parts[1]) {
        throw new Error(`invalid repo ${repo}, expected owner/name`);
    }
    return [parts[0], parts[1]];
}

async function apiReleaseByTag(
    owner: string,
    name: string,
    tag: string,
    token: string,
): Promise<GithubRelease | null> {
    const response = await fetch(
        `https://api.github.com/repos/${owner}/${name}/releases/tags/${encodeURIComponent(tag)}`,
        { headers: githubHeaders(token) },
    );
    if (response.status === 404) return null;
    if (!response.ok) {
        throw new Error(`GitHub API ${response.status}: ${await response.text()}`);
    }
    return (await response.json()) as GithubRelease;
}

async function apiCreateRelease(
    owner: string,
    name: string,
    tag: string,
    title: string,
    body: string,
    draft: boolean,
    token: string,
): Promise<GithubRelease> {
    const response = await fetch(`https://api.github.com/repos/${owner}/${name}/releases`, {
        method: "POST",
        headers: { ...githubHeaders(token), "Content-Type": "application/json" },
        body: JSON.stringify({ tag_name: tag, name: title, body, draft }),
    });
    if (!response.ok) {
        throw new Error(`GitHub API ${response.status}: ${await response.text()}`);
    }
    return (await response.json()) as GithubRelease;
}

async function apiUploadAsset(
    owner: string,
    name: string,
    releaseId: number,
    assetName: string,
    filePath: string,
    token: string,
): Promise<void> {
    const url = `https://uploads.github.com/repos/${owner}/${name}/releases/${releaseId}/assets?name=${encodeURIComponent(assetName)}`;
    const body = readFileSync(filePath);
    const response = await fetch(url, {
        method: "POST",
        headers: {
            ...githubHeaders(token),
            "Content-Type": "application/octet-stream",
            "Content-Length": String(body.length),
        },
        body,
    });
    const text = await response.text();
    if (response.ok) {
        return;
    }
    if (response.status === 422 && text.includes("already_exists")) {
        console.log(`release: skip ${assetName} (already uploaded)`);
        return;
    }
    throw new Error(`GitHub upload ${response.status}: ${text}`);
}

function githubHeaders(token: string): Record<string, string> {
    return {
        Accept: "application/vnd.github+json",
        Authorization: `Bearer ${token}`,
        "User-Agent": USER_AGENT,
        "X-GitHub-Api-Version": "2022-11-28",
    };
}

function git(cwd: string, args: string[]): void {
    const result = spawnSync("git", args, { cwd, stdio: "inherit" });
    if (result.status !== 0) {
        throw new Error(`git ${args.join(" ")} failed`);
    }
}
