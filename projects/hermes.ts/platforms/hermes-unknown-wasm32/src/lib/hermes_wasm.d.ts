/* tslint:disable */
/* eslint-disable */

/**
 * Contract registry exposed to JavaScript hosts.
 */
export class HermesRegistry {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Describe one registered method, or return an error JSON document.
     */
    describe_method(service: string, method: string): string;
    /**
     * Create an empty registry.
     */
    constructor();
    /**
     * Number of registered operations.
     */
    operation_count(): number;
    /**
     * Register a `vos/hermes.contract/v0` artifact JSON document.
     */
    register_contract_artifact(json: string): string;
    /**
     * Number of registered services.
     */
    service_count(): number;
    /**
     * Registered service names as a JSON array.
     */
    service_names(): string;
}

/**
 * Decode a `HermesDispatchRequest` and return its canonical JSON form.
 */
export function decodeDispatchRequest(json: string): string;

/**
 * Decode a `HermesDispatchResponse` and return its canonical JSON form.
 */
export function encodeDispatchResponse(json: string): string;

/**
 * Contract format accepted by Hermes.
 */
export function hermesContractFormat(): string;

/**
 * Hermes WASM binding version (Cargo package version).
 */
export function hermesVersion(): string;

/**
 * Load and validate a `vos/hermes.contract/v0` artifact JSON document.
 */
export function validateContractArtifact(json: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_hermesregistry_free: (a: number, b: number) => void;
    readonly decodeDispatchRequest: (a: number, b: number) => [number, number];
    readonly encodeDispatchResponse: (a: number, b: number) => [number, number];
    readonly hermesContractFormat: () => [number, number];
    readonly hermesVersion: () => [number, number];
    readonly hermesregistry_describe_method: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly hermesregistry_new: () => number;
    readonly hermesregistry_operation_count: (a: number) => number;
    readonly hermesregistry_register_contract_artifact: (a: number, b: number, c: number) => [number, number];
    readonly hermesregistry_service_count: (a: number) => number;
    readonly hermesregistry_service_names: (a: number) => [number, number];
    readonly validateContractArtifact: (a: number, b: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
