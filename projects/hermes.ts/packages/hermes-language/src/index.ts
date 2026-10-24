/**
 * Browser-facing Hermes RPC surface.
 *
 * Re-exports the `@yydb/hermes-unknown-wasm32` binding and adds a cached
 * `loadHermesWeb` entry point for hosts that want one ready-to-use promise.
 * No RPC semantics live here: contract validation, registry queries and wire
 * codecs stay in the Rust `hermes-wasm` crate.
 */
import {
    initWasm,
    hermesVersion,
    resetWasmBindingForTests,
    type InitWasmOptions,
} from "@yydb/hermes-unknown-wasm32";

export * from "@yydb/hermes-unknown-wasm32";
export type { InitWasmOptions, WasmInitInput } from "@yydb/hermes-unknown-wasm32";

/** A loaded Hermes WASM binding. */
export interface HermesWeb {
    /** Hermes WASM binding version. */
    readonly version: string;
}

let webPromise: Promise<HermesWeb> | null = null;

/**
 * Initialize the WASM binding once and cache the result.
 *
 * Repeated calls return the same promise, so concurrent callers share one
 * instantiation. A failed initialization clears the cache and can be retried.
 */
export function loadHermesWeb(options: InitWasmOptions = {}): Promise<HermesWeb> {
    webPromise ??= (async () => {
        await initWasm(options);
        return { version: hermesVersion() };
    })().catch((error: unknown) => {
        webPromise = null;
        throw error;
    });
    return webPromise;
}

/** @internal Reset the cached web binding and init gate (tests only). */
export function resetHermesWebForTests(): void {
    webPromise = null;
    resetWasmBindingForTests();
}
