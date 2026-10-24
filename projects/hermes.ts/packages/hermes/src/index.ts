/**
 * `@yydb/hermes` — Node.js facade for the Hermes RPC runtime.
 *
 * Resolves the prebuilt N-API binary for the current host from
 * `@yydb/hermes-<platform>` and re-exports its dispatch surface. Browser,
 * edge and WASI hosts use `@yydb/hermes-language` instead.
 *
 * No RPC semantics live here: artifact loading, routing and lifecycle stay in
 * the Rust `hermes-runtime` crate exposed through `hermes-napi`.
 */
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

/** Prebuilt platforms with a published N-API package. */
export const HERMES_PLATFORM_PACKAGES = {
    "win32-x64": "@yydb/hermes-win32-x64",
    "linux-x64": "@yydb/hermes-linux-x64",
    "linux-arm64": "@yydb/hermes-linux-arm64",
    "darwin-x64": "@yydb/hermes-darwin-x64",
    "darwin-arm64": "@yydb/hermes-darwin-arm64",
} as const;

/** Structured RPC error returned in a dispatch result. */
export type HermesRpcError = {
    /** Stable RPC error code (kebab-case), see the Rust `RpcErrorCode`. */
    code: string;
    /** Upstream artifact diagnostic code (`ART010`, `ART020`, `SVC013`, ...) when present. */
    diagnosticCode?: string | null;
    /** Human-readable detail. Branch on `code`, not on this text. */
    message: string;
};

/**
 * Result of one dispatch.
 *
 * `result` (unary), `stream` (server-stream) and `error` are mutually
 * exclusive. JSON values are stringified so the JS facade does not introduce a
 * second wire codec.
 */
export type HermesDispatchResult = {
    /** Service that was addressed. */
    service: string;
    /** Method that was addressed. */
    method: string;
    /** Durable operation id resolved by the registry, or `null` when routing failed. */
    operationId: string | null;
    /** Durable method id resolved by the registry, or `null` when routing failed. */
    methodId: string | null;
    /** Unary result JSON, or `null`. */
    result: string | null;
    /** Server-stream element JSON strings, or `null`. */
    stream: string[] | null;
    /** Structured error, or `null` on success. */
    error: HermesRpcError | null;
};

/** Native dispatcher handle exposed by `@yydb/hermes-<platform>`. */
export type HermesNativeDispatcherHandle = {
    loadContractArtifact(json: string): void;
    setMaxConcurrency(limit?: number | null): void;
    dispatch(service: string, method: string, paramsJson: string, timeoutMs?: number | null): HermesDispatchResult;
};

/** Native binding surface shared by every `@yydb/hermes-<platform>` package. */
export type HermesNativeBindings = {
    hermesVersion(): string;
    runHermesDispatch(
        contractArtifactJson: string,
        service: string,
        method: string,
        paramsJson: string,
        timeoutMs?: number | null,
    ): HermesDispatchResult;
    HermesDispatcherHandle: new () => HermesNativeDispatcherHandle;
};

/**
 * Resolve the native package for one platform/arch pair.
 *
 * Throws when no package is published for that host instead of returning an
 * empty surface, so an unsupported runtime fails loudly at first use.
 */
export function resolvePlatformPackage(platform: NodeJS.Platform | string, arch: string): string {
    const key = `${platform}-${arch}`;
    const packageName = (HERMES_PLATFORM_PACKAGES as Record<string, string | undefined>)[key];
    if (!packageName) {
        throw new Error(
            `@yydb/hermes: no native binding published for ${key} (supported: ${Object.keys(HERMES_PLATFORM_PACKAGES).join(", ")})`,
        );
    }
    return packageName;
}

function loadModule(packageName: string): HermesNativeBindings {
    const loaded = require(packageName) as Partial<HermesNativeBindings> & { default?: Partial<HermesNativeBindings> };
    const module = (loaded.default ?? loaded) as Partial<HermesNativeBindings>;
    if (
        typeof module.hermesVersion !== "function" ||
        typeof module.runHermesDispatch !== "function" ||
        typeof module.HermesDispatcherHandle !== "function"
    ) {
        throw new Error(`@yydb/hermes: ${packageName} does not expose the expected Hermes native exports`);
    }
    return module as HermesNativeBindings;
}

let cached: HermesNativeBindings | undefined;

/**
 * Load the N-API binding for the current host.
 *
 * Throws a descriptive error when the host platform is unsupported or the
 * matching optional dependency was not installed.
 */
export function loadHermesNative(): HermesNativeBindings {
    if (cached) {
        return cached;
    }
    const packageName = resolvePlatformPackage(process.platform, process.arch);
    try {
        cached = loadModule(packageName);
    } catch (error) {
        if (error instanceof Error && error.message.startsWith("@yydb/hermes:")) {
            throw error;
        }
        const message = error instanceof Error ? error.message : String(error);
        throw new Error(
            `@yydb/hermes: native binding for ${process.platform}-${process.arch} not installed (${packageName}: ${message}). Reinstall @yydb/hermes so its optional dependencies resolve.`,
        );
    }
    return cached;
}

/** Hermes N-API binding version. */
export function hermesVersion(): string {
    return loadHermesNative().hermesVersion();
}

/** Load one Hermes contract artifact and dispatch `service.method` in one call. */
export function runHermesDispatch(
    contractArtifactJson: string,
    service: string,
    method: string,
    paramsJson: string,
    timeoutMs?: number | null,
): HermesDispatchResult {
    return loadHermesNative().runHermesDispatch(contractArtifactJson, service, method, paramsJson, timeoutMs);
}

/** Persistent dispatcher handle backed by the native binding. */
export class HermesDispatcherHandle {
    #inner: HermesNativeDispatcherHandle;

    constructor() {
        const NativeHandle = loadHermesNative().HermesDispatcherHandle;
        this.#inner = new NativeHandle();
    }

    /** Load a `vos/hermes.contract/v0` artifact JSON document. */
    loadContractArtifact(json: string): void {
        this.#inner.loadContractArtifact(json);
    }

    /** Set the in-flight dispatch limit. `null` or `0` means unlimited. */
    setMaxConcurrency(limit?: number | null): void {
        this.#inner.setMaxConcurrency(limit);
    }

    /** Dispatch by `service` name + `method` name. */
    dispatch(service: string, method: string, paramsJson: string, timeoutMs?: number | null): HermesDispatchResult {
        return this.#inner.dispatch(service, method, paramsJson, timeoutMs);
    }
}
