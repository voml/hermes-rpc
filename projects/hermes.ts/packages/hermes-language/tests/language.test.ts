import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const entry = new URL("../src/index.ts", import.meta.url).href;
const fixture = new URL(
    "../../../platforms/hermes-unknown-wasm32/tests/fixtures/operations_services.contract.json",
    import.meta.url,
);
const CONTRACT_ARTIFACT = readFileSync(fixture, "utf8");

test("loadHermesWeb initializes the real wasm binding", async () => {
    const { loadHermesWeb } = await import(entry);
    const web = await loadHermesWeb();
    assert.ok(web.version.length > 0);
});

test("loadHermesWeb caches one initialization", async () => {
    const { loadHermesWeb } = await import(entry);
    const first = await loadHermesWeb();
    const second = await loadHermesWeb();
    assert.equal(second, first);
});

test("re-exported helpers delegate to the wasm binding", async () => {
    const { loadHermesWeb, validateContractArtifact, HermesRegistry, decodeDispatchRequest } = await import(entry);
    await loadHermesWeb();

    assert.deepEqual(JSON.parse(validateContractArtifact(CONTRACT_ARTIFACT)), { ok: true });
    assert.equal(JSON.parse(validateContractArtifact('{"version":"vos/hermes.contract/v0"}')).ok, false);

    const registry = new HermesRegistry();
    try {
        assert.deepEqual(JSON.parse(registry.registerContractArtifact(CONTRACT_ARTIFACT)), { ok: true });
        assert.deepEqual(JSON.parse(registry.serviceNames()), ["IdentityService"]);
    } finally {
        registry.free();
    }

    const request = JSON.parse(
        decodeDispatchRequest('{"service":"IdentityService","method":"create","params":{}}'),
    ) as { service: string; method: string };
    assert.equal(request.service, "IdentityService");
    assert.equal(request.method, "create");
});

test("resetHermesWebForTests re-arms the initialization gate", async () => {
    const { loadHermesWeb, resetHermesWebForTests, hermesVersion } = await import(entry);
    await loadHermesWeb();
    resetHermesWebForTests();
    assert.throws(() => hermesVersion(), /call initWasm\(\)/);
    const web = await loadHermesWeb();
    assert.ok(web.version.length > 0);
});
