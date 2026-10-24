import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { SERVICE_CONTRACT, SERVICE_PROJECTION, mutateFixture } from "./fixtures.ts";

const entry = new URL("../src/index.ts", import.meta.url).href;

type ValidationResult = { ok: boolean; code?: string; message?: string };

function parse(raw: string): ValidationResult {
    return JSON.parse(raw) as ValidationResult;
}

test("wasm artifact lives under src/lib/", () => {
    readFileSync(new URL("../src/lib/hermes_wasm_bg.wasm", import.meta.url));
});

test("exports throw before initWasm", async () => {
    const { hermesVersion, resetWasmBindingForTests } = await import(entry);
    resetWasmBindingForTests();
    assert.throws(() => hermesVersion(), /call initWasm\(\) before Hermes WASM methods/);
});

test("initWasm exposes the real version", async () => {
    const { hermesVersion, initWasm } = await import(entry);
    await initWasm();
    const version = hermesVersion();
    assert.ok(version.length > 0);
});

test("validateServiceContract accepts valid and rejects invalid contracts", async () => {
    const { initWasm, validateServiceContract } = await import(entry);
    await initWasm();

    assert.deepEqual(parse(validateServiceContract(SERVICE_CONTRACT)), { ok: true });

    const invalid = parse(validateServiceContract('{"service_name":42,"methods":[]}'));
    assert.equal(invalid.ok, false);
    assert.equal(invalid.code, "invalid-request");
});

test("validateServiceProjection accepts the upstream fixture and rejects a bad stage", async () => {
    const { initWasm, validateServiceProjection } = await import(entry);
    await initWasm();

    assert.deepEqual(parse(validateServiceProjection(SERVICE_PROJECTION)), { ok: true });

    const badStage = mutateFixture(SERVICE_PROJECTION, (value) => {
        const envelope = value.envelope as Record<string, unknown>;
        envelope.stage = "resolved-contract";
    });
    const invalid = parse(validateServiceProjection(badStage));
    assert.equal(invalid.ok, false);
    assert.equal(invalid.code, "SRV011");
});

test("HermesRegistry registers contracts and describes methods", async () => {
    const { HermesRegistry, initWasm } = await import(entry);
    await initWasm();

    const registry = new HermesRegistry();
    try {
        assert.deepEqual(parse(registry.registerServiceContract(SERVICE_CONTRACT)), { ok: true });
        assert.equal(registry.serviceCount(), 1);
        assert.deepEqual(JSON.parse(registry.serviceNames()), ["hermes.rpc.FileService"]);

        const descriptor = JSON.parse(registry.describeMethod("hermes.rpc.FileService", "read")) as {
            name: string;
            request_type: string;
        };
        assert.equal(descriptor.name, "read");
        assert.equal(descriptor.request_type, "ReadRequest");

        const duplicate = parse(registry.registerServiceContract(SERVICE_CONTRACT));
        assert.equal(duplicate.ok, false);
        assert.equal(duplicate.code, "duplicate-registration");
    } finally {
        registry.free();
    }
});

test("HermesRegistry registers service projections", async () => {
    const { HermesRegistry, initWasm } = await import(entry);
    await initWasm();

    const registry = new HermesRegistry();
    try {
        assert.deepEqual(parse(registry.registerServiceProjection(SERVICE_PROJECTION)), { ok: true });
        assert.deepEqual(JSON.parse(registry.serviceNames()), ["demo.auth.IdentityService"]);
    } finally {
        registry.free();
    }
});

test("dispatch wire helpers decode and encode envelopes", async () => {
    const { decodeDispatchRequest, encodeDispatchResponse, initWasm } = await import(entry);
    await initWasm();

    const request = JSON.parse(decodeDispatchRequest('{"operation":"op_0123456789abcdef","params":{"a":1}}')) as {
        operation: string;
        params: unknown;
    };
    assert.equal(request.operation, "op_0123456789abcdef");
    assert.deepEqual(request.params, { a: 1 });

    const response = JSON.parse(
        encodeDispatchResponse('{"result":{"value":1},"operationId":"op_0123456789abcdef"}'),
    ) as { operationId: string; result: unknown };
    assert.equal(response.operationId, "op_0123456789abcdef");
    assert.deepEqual(response.result, { value: 1 });

    const invalid = parse(encodeDispatchResponse('{"operationId":"op_0123456789abcdef"}'));
    assert.equal(invalid.ok, false);
    assert.equal(invalid.code, "invalid-request");
});

test("initWasm is idempotent", async () => {
    const { hermesVersion, initWasm } = await import(entry);
    await initWasm();
    const first = hermesVersion();
    await initWasm();
    assert.equal(hermesVersion(), first);
});
