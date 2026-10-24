# @yydb/hermes

Hermes RPC runtime — VOS operation artifact loader and dispatch for Node.js.

Provides platform-specific N-API binaries (Linux, macOS, Windows). Browser,
edge and WASI hosts use `@yydb/hermes-language`, which wraps the Rust
`hermes-wasm` binding. `@yydb/hermes` does not depend on either package.

## Usage

```ts
import { Hermes } from "@yydb/hermes";
```

## License

MPL-2.0
