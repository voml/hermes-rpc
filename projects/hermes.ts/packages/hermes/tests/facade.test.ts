import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const pkgRoot = new URL("..", import.meta.url);
const pkg = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")) as {
    main: string;
    types: string;
    exports: Record<string, Record<string, string>>;
};
const entry = new URL("../src/index.ts", import.meta.url).href;

function pkgPath(relative: string): string {
    return fileURLToPath(new URL(relative, pkgRoot));
}

test("declared entrypoints point at existing build output", () => {
    for (const relative of [pkg.main, pkg.types, pkg.exports["."].types, pkg.exports["."].import, pkg.exports["."].default]) {
        assert.equal(typeof relative, "string", "entrypoint must be a path");
        assert.ok(existsSync(pkgPath(relative)), `missing ${relative}; run \`node ./scripts/build-dist.mjs\``);
    }
    assert.ok(existsSync(pkgPath("src/index.ts")), "src facade must ship with the package");
});

test("resolvePlatformPackage maps every supported host", async () => {
    const { resolvePlatformPackage } = await import(entry);
    assert.equal(resolvePlatformPackage("win32", "x64"), "@yydb/hermes-win32-x64");
    assert.equal(resolvePlatformPackage("linux", "x64"), "@yydb/hermes-linux-x64");
    assert.equal(resolvePlatformPackage("linux", "arm64"), "@yydb/hermes-linux-arm64");
    assert.equal(resolvePlatformPackage("darwin", "x64"), "@yydb/hermes-darwin-x64");
    assert.equal(resolvePlatformPackage("darwin", "arm64"), "@yydb/hermes-darwin-arm64");
});

test("resolvePlatformPackage rejects unsupported hosts with a clear error", async () => {
    const { resolvePlatformPackage } = await import(entry);
    assert.throws(
        () => resolvePlatformPackage("freebsd", "x64"),
        (error) =>
            error instanceof Error &&
            error.message.includes("freebsd-x64") &&
            error.message.includes("win32-x64"),
    );
});

test("hermesVersion returns a non-empty native version", async (t) => {
    const { resolvePlatformPackage, hermesVersion } = await import(entry);
    const packageName = resolvePlatformPackage(process.platform, process.arch);
    try {
        require.resolve(packageName);
    } catch {
        t.skip(`optional native package ${packageName} is not installed`);
        return;
    }
    const version = hermesVersion();
    assert.equal(typeof version, "string");
    assert.ok(version.length > 0);
});
