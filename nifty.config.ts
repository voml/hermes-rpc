// Nifty project configuration for hermes-rpc.
// Trusted Publisher contract: repo voml/hermes-rpc, workflow publish-npm.yml, environment NPM_PUBLISH.
import { defineConfig } from "@doki-land/nifty";

export default defineConfig({
    trust: {
        npm: {
            repo: "voml/hermes-rpc",
            file: "publish-npm.yml",
            environment: "NPM_PUBLISH",
        },
    },
    publish: {
        packages: [
            "@yydb/hermes",
            "@yydb/hermes-skills",
            "@yydb/hermes-unknown-wasm32",
            "@yydb/hermes-win32-x64",
            "@yydb/hermes-linux-x64",
            "@yydb/hermes-linux-arm64",
            "@yydb/hermes-darwin-x64",
            "@yydb/hermes-darwin-arm64",
        ],
    },
});
