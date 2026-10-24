import assert from "node:assert/strict";
import { test } from "node:test";

const entry = new URL("../src/index.ts", import.meta.url).href;

const SERVICE_CONTRACT = JSON.stringify({
    service_name: "hermes.rpc.FileService",
    methods: [{ name: "read", request_type: "ReadRequest", response_type: "ReadResponse", docs: "" }],
});

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
    const { loadHermesWeb, validateServiceContract, HermesRegistry, decodeDispatchRequest } = await import(entry);
    await loadHermesWeb();

    assert.deepEqual(JSON.parse(validateServiceContract(SERVICE_CONTRACT)), { ok: true });
    assert.equal(JSON.parse(validateServiceContract('{"service_name":42}')).ok, false);

    const registry = new HermesRegistry();
    try {
        assert.deepEqual(JSON.parse(registry.registerServiceContract(SERVICE_CONTRACT)), { ok: true });
        assert.deepEqual(JSON.parse(registry.serviceNames()), ["hermes.rpc.FileService"]);
    } finally {
        registry.free();
    }

    const request = JSON.parse(decodeDispatchRequest('{"operation":"op_0123456789abcdef"}')) as {
        operation: string;
    };
    assert.equal(request.operation, "op_0123456789abcdef");
});

test("resetHermesWebForTests re-arms the initialization gate", async () => {
    const { loadHermesWeb, resetHermesWebForTests, hermesVersion } = await import(entry);
    await loadHermesWeb();
    resetHermesWebForTests();
    assert.throws(() => hermesVersion(), /call initWasm\(\)/);
    const web = await loadHermesWeb();
    assert.ok(web.version.length > 0);
});
