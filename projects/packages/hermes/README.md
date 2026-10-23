# @yydb/hermes

Hermes RPC runtime — VOS operation artifact loader and dispatch for Node.js.

Provides a unified facade over platform-specific N-API binaries (Linux, macOS, Windows) with a WASM fallback (`@yydb/hermes-unknown-wasm32`) for browsers and WASI runtimes.

## Usage

```ts
import { Hermes } from "@yydb/hermes";
```

## License

MPL-2.0
