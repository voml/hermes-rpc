# @yydb/hermes

Hermes RPC runtime — VOS operation artifact loader and dispatch for Node.js.

Provides platform-specific N-API binaries (Linux, macOS, Windows). Browser,
edge and WASI hosts use `@yydb/hermes-language`, which wraps the Rust
`hermes-wasm` binding. `@yydb/hermes` does not depend on either package.

## Usage

```ts
import { HermesDispatcherHandle, hermesVersion, runHermesDispatch } from "@yydb/hermes";

// One-shot: load one contract artifact and dispatch.
const result = runHermesDispatch(contractArtifactJson, "IdentityService", "create", "{}");
if (result.error) {
    console.error(result.error.code, result.error.message);
} else {
    console.log(result.result);
}

// Persistent handle: load the contract once, dispatch many times.
const dispatcher = new HermesDispatcherHandle();
dispatcher.loadContractArtifact(contractArtifactJson);
dispatcher.setMaxConcurrency(8);
dispatcher.dispatch("IdentityService", "create", "{}", 5000);

console.log(hermesVersion());
```

The matching `@yydb/hermes-<platform>` package is resolved from the current
host at load time. Unsupported platforms and missing optional dependencies
throw an explicit error instead of returning an empty facade.

## License

MPL-2.0
