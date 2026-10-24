# @yydb/hermes-language

Browser-facing Hermes RPC helpers backed by the Rust `hermes-wasm` binding.

This package re-exports `@yydb/hermes-unknown-wasm32` and adds
`loadHermesWeb()`, which initializes the WASM module once and caches the
result. Contract validation, service registry queries and dispatch wire codecs
all run in Rust, so no TypeScript parser is involved.

## Usage

```ts
import { loadHermesWeb, validateServiceContract, HermesRegistry } from "@yydb/hermes-language";

const { version } = await loadHermesWeb();
console.log(version);

const result = validateServiceContract('{"service_name":"demo.Echo","methods":[]}');

const registry = new HermesRegistry();
registry.registerServiceContract('{"service_name":"demo.Echo","methods":[]}');
console.log(registry.serviceNames());
registry.free();
```

`@yydb/hermes` (the Node.js N-API package) does not depend on this package.
Use it in browsers, edge runtimes and WASI hosts where native binaries are
unavailable.

## Result documents

Validation and registry mutations return `{"ok":true}` or
`{"ok":false,"code":"<stable-code>","message":"<detail>"}`.

## License

MPL-2.0
