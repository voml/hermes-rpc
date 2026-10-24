import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import {
    CONTRACT_ARTIFACT,
    CREATE_METHOD,
    CREATE_METHOD_ID,
    CREATE_OPERATION_ID,
    LOOKUP_METHOD,
    SERVICE_NAME,
    mutateFixture,
} from "./fixtures.ts";

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

test("initWasm exposes the real version and contract format", async () => {
    const { hermesVersion, hermesContractFormat, initWasm } = await import(entry);
    await initWasm();
    assert.ok(hermesVersion().length > 0);
    assert.equal(hermesContractFormat(), "vos/hermes.contract/v0");
});

test("validateContractArtifact accepts the Hermes fixture and rejects other formats", async () => {
    const { initWasm, validateContractArtifact } = await import(entry);
    await initWasm();

    assert.deepEqual(parse(validateContractArtifact(CONTRACT_ARTIFACT)), { ok: true });

    const iris = mutateFixture(CONTRACT_ARTIFACT, (value) => {
        value.version = "vos/iris.contract/v0";
    });
    const wrongFormat = parse(validateContractArtifact(iris));
    assert.equal(wrongFormat.ok, false);
    assert.equal(wrongFormat.code, "artifact-corrupt");
    assert.equal((wrongFormat as ValidationResult & { diagnosticCode?: string }).diagnosticCode, "ART010");

    const unknownField = mutateFixture(CONTRACT_ARTIFACT, (value) => {
        value.futureField = true;
    });
    assert.equal(parse(validateContractArtifact(unknownField)).ok, false);
});

test("validateContractArtifact rejects fingerprint mismatches", async () => {
    const { initWasm, validateContractArtifact } = await import(entry);
    await initWasm();

    const mismatch = mutateFixture(CONTRACT_ARTIFACT, (value) => {
        value.fingerprint = "0".repeat(64);
    });
    const invalid = parse(validateContractArtifact(mismatch)) as ValidationResult & { diagnosticCode?: string };
    assert.equal(invalid.ok, false);
    assert.equal(invalid.diagnosticCode, "ART020");
});

test("HermesRegistry registers contracts and describes methods", async () => {
    const { HermesRegistry, initWasm } = await import(entry);
    await initWasm();

    const registry = new HermesRegistry();
    try {
        assert.deepEqual(parse(registry.registerContractArtifact(CONTRACT_ARTIFACT)), { ok: true });
        assert.equal(registry.serviceCount(), 1);
        assert.equal(registry.operationCount(), 2);
        assert.deepEqual(JSON.parse(registry.serviceNames()), [SERVICE_NAME]);

        const descriptor = JSON.parse(registry.describeMethod(SERVICE_NAME, CREATE_METHOD)) as {
            service: string;
            method: string;
            methodId: string;
            operationId: string;
            stream: string;
        };
        assert.equal(descriptor.method, CREATE_METHOD);
        assert.equal(descriptor.methodId, CREATE_METHOD_ID);
        assert.equal(descriptor.operationId, CREATE_OPERATION_ID);
        assert.equal(descriptor.stream, "unary");

        const stream = JSON.parse(registry.describeMethod(SERVICE_NAME, LOOKUP_METHOD)) as { stream: string };
        assert.equal(stream.stream, "server-stream");

        const duplicate = parse(registry.registerContractArtifact(CONTRACT_ARTIFACT));
        assert.equal(duplicate.ok, false);
        assert.equal(duplicate.code, "duplicate-registration");
    } finally {
        registry.free();
    }
});

test("HermesRegistry reports unknown services and methods", async () => {
    const { HermesRegistry, initWasm } = await import(entry);
    await initWasm();

    const registry = new HermesRegistry();
    try {
        registry.registerContractArtifact(CONTRACT_ARTIFACT);
        const service = parse(registry.describeMethod("MissingService", CREATE_METHOD));
        assert.equal(service.code, "unknown-service");
        const method = parse(registry.describeMethod(SERVICE_NAME, "missing"));
        assert.equal(method.code, "unknown-method");
    } finally {
        registry.free();
    }
});

test("dispatch wire helpers decode and encode envelopes", async () => {
    const { decodeDispatchRequest, encodeDispatchResponse, initWasm } = await import(entry);
    await initWasm();

    const request = JSON.parse(
        decodeDispatchRequest('{"service":"IdentityService","method":"create","params":{"request":{}}}'),
    ) as { service: string; method: string; params: unknown };
    assert.equal(request.service, "IdentityService");
    assert.equal(request.method, "create");
    assert.deepEqual(request.params, { request: {} });

    const response = JSON.parse(
        encodeDispatchResponse(
            '{"service":"IdentityService","method":"create","operationId":"op_38ecee3afe0deee18f4f4c89f7b33c390d84844a3bf0832632f2f612cd132437","methodId":"mtd_a1195cc42bc0418387f233d5db60833493a2aecab3923e75c97be3509f0aecfd","result":{"id":"x"}}',
        ),
    ) as { operationId: string; result: unknown };
    assert.equal(response.operationId, CREATE_OPERATION_ID);
    assert.deepEqual(response.result, { id: "x" });

    const invalid = parse(encodeDispatchResponse('{"service":"IdentityService","method":"create"}'));
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
