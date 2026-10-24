/** WASM bytes or module handle for custom asset pipelines. */
export type WasmInitInput = URL | Request | Response | ArrayBuffer | Uint8Array | WebAssembly.Module;

export type InitWasmOptions = {
    /** When omitted, loads the bundled `lib/hermes_wasm_bg.wasm` asset. */
    module?: WasmInitInput;
};

type GlueRegistry = {
    describe_method(service: string, method: string): string;
    register_service_contract(json: string): string;
    register_service_projection(json: string): string;
    service_count(): number;
    service_names(): string;
    free(): void;
};

type GlueModule = {
    default: (input: { module_or_path: WasmInitInput | ArrayBuffer | Uint8Array }) => Promise<unknown>;
    HermesRegistry?: new () => GlueRegistry;
    hermesVersion?: () => string;
    validateServiceContract?: (json: string) => string;
    validateServiceProjection?: (json: string) => string;
    decodeDispatchRequest?: (json: string) => string;
    encodeDispatchResponse?: (json: string) => string;
};

let glue: GlueModule | null = null;
let ready = false;

function assertReady(): void {
    if (!ready || glue == null) {
        throw new Error("@yydb/hermes-unknown-wasm32: call initWasm() before Hermes WASM methods");
    }
}

function glueApi(): GlueModule {
    assertReady();
    return glue!;
}

function requireExport<K extends keyof Omit<GlueModule, "default">>(name: K): NonNullable<GlueModule[K]> {
    const api = glueApi();
    const value = api[name];
    if (value == null) {
        throw new Error(`@yydb/hermes-unknown-wasm32: ${String(name)} export missing; rebuild wasm artifacts`);
    }
    return value as NonNullable<GlueModule[K]>;
}

async function loadGlueModule(): Promise<GlueModule> {
    if (glue) {
        return glue;
    }
    const mod = (await import("./lib/hermes_wasm.js")) as GlueModule;
    glue = mod;
    return mod;
}

async function resolveDefaultWasmBytes(): Promise<ArrayBuffer | Uint8Array | URL> {
    const wasmUrl = new URL("./lib/hermes_wasm_bg.wasm", import.meta.url);
    if (typeof process !== "undefined" && process.versions?.node) {
        const { readFileSync } = await import("node:fs");
        const { fileURLToPath } = await import("node:url");
        return readFileSync(fileURLToPath(wasmUrl));
    }
    return wasmUrl;
}

async function normalizeInitInput(input: WasmInitInput | undefined): Promise<WasmInitInput | ArrayBuffer | Uint8Array> {
    if (input === undefined) {
        return resolveDefaultWasmBytes();
    }
    if (
        input instanceof URL ||
        input instanceof Request ||
        input instanceof Response ||
        input instanceof ArrayBuffer ||
        input instanceof Uint8Array ||
        input instanceof WebAssembly.Module
    ) {
        return input;
    }
    throw new Error(`@yydb/hermes-unknown-wasm32: unsupported WASM init input (${typeof input})`);
}

/** One-time WASM init. Required before registry and validation methods. */
export async function initWasm(options: InitWasmOptions = {}): Promise<void> {
    if (ready) {
        return;
    }
    const module = await loadGlueModule();
    const moduleOrPath = await normalizeInitInput(options.module);
    await module.default({ module_or_path: moduleOrPath });
    ready = true;
}

/** Hermes WASM binding version. */
export function hermesVersion(): string {
    return requireExport("hermesVersion")();
}

/** Validate a `HermesServiceContract` JSON document. */
export function validateServiceContract(json: string): string {
    return requireExport("validateServiceContract")(json);
}

/** Validate an upstream VOS `ServiceProjectionArtifact` JSON document. */
export function validateServiceProjection(json: string): string {
    return requireExport("validateServiceProjection")(json);
}

/** Decode a dispatch request and return its canonical JSON form. */
export function decodeDispatchRequest(json: string): string {
    return requireExport("decodeDispatchRequest")(json);
}

/** Decode a dispatch response and return its canonical JSON form. */
export function encodeDispatchResponse(json: string): string {
    return requireExport("encodeDispatchResponse")(json);
}

/** Service registry backed by the Rust `BTreeServiceRegistry`. */
export class HermesRegistry {
    #inner: GlueRegistry;

    constructor() {
        const Registry = requireExport("HermesRegistry");
        this.#inner = new Registry();
    }

    /** Register a `HermesServiceContract` JSON document. Returns a result JSON document. */
    registerServiceContract(json: string): string {
        return this.#inner.register_service_contract(json);
    }

    /** Register every service in a `ServiceProjectionArtifact` JSON document. */
    registerServiceProjection(json: string): string {
        return this.#inner.register_service_projection(json);
    }

    /** Number of registered services. */
    serviceCount(): number {
        return this.#inner.service_count();
    }

    /** Registered service names as a JSON array string. */
    serviceNames(): string {
        return this.#inner.service_names();
    }

    /** Describe one registered method, or return an error JSON document. */
    describeMethod(service: string, method: string): string {
        return this.#inner.describe_method(service, method);
    }

    /** Release the wasm-owned registry handle. */
    free(): void {
        this.#inner.free();
    }
}

/** @internal Reset init gate (binding tests only). */
export function resetWasmBindingForTests(): void {
    ready = false;
    glue = null;
}
