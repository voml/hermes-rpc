# @yydb/hermes-unknown-wasm32

Browser WebAssembly binding for the Hermes RPC runtime, generated from the
Rust `hermes-wasm` crate with `wasm-pack`. The binding delegates contract
validation, service registry queries and dispatch wire codecs to
`hermes-runtime`, so TypeScript hosts do not carry a parallel parser.

## Usage

```ts
import {
    HermesRegistry,
    initWasm,
    hermesVersion,
    validateContractArtifact,
} from "@yydb/hermes-unknown-wasm32";

await initWasm();

console.log(hermesVersion());
console.log(validateContractArtifact(hermesContractArtifactJson));

const registry = new HermesRegistry();
registry.registerContractArtifact(hermesContractArtifactJson);
console.log(registry.serviceNames());
registry.free();
```

`initWasm()` loads the bundled `src/lib/hermes_wasm_bg.wasm` asset. Node reads
it from disk, browsers load it from the module URL. Pass `{ module }` to supply
custom WASM bytes.

## Result documents

Validation accepts only `vos/hermes.contract/v0`. Unknown formats, versions
and fields, fingerprint mismatches and missing producer metadata are rejected
with the upstream diagnostic code (`ART010` / `ART020` / `ART021`).

Validation and registry mutations return JSON:

- success: `{"ok":true}`
- failure: `{"ok":false,"code":"<stable-code>","diagnosticCode":"<upstream-code>","message":"<detail>"}`

`decodeDispatchRequest` and `encodeDispatchResponse` return the canonical JSON
form of the wire envelope, or the same failure document.

## Building

```bash
node scripts/build-wasm.mjs   # from the repository root
pnpm --filter @yydb/hermes-unknown-wasm32 build
```

`build:wasm` requires `wasm-pack` and the `wasm32-unknown-unknown` target.

## License

MPL-2.0
