export const SERVICE_CONTRACT = JSON.stringify({
    service_name: "hermes.rpc.FileService",
    methods: [
        {
            name: "read",
            request_type: "ReadRequest",
            response_type: "ReadResponse",
            docs: "Read one file.",
        },
    ],
    revision: "rev-1",
});

const SOURCE =
    "namespace demo::auth\n\nservice IdentityService {\n  create(request: CreateRequest) -> Identity\n  delete(request: DeleteRequest) -> bool\n}\n";

export const SERVICE_PROJECTION = JSON.stringify({
    envelope: {
        stage: "service-syntax-projection",
        contractFormatVersion: "vos-service-contract-v0",
        languageVersion: "vos-language-v0",
        canonicalizationVersion: "vos-canonical-v0",
        sourceUnits: [{ sourceUnitId: 0, source: SOURCE }],
    },
    services: [
        {
            name: "IdentityService",
            canonicalPath: ["demo", "auth", "IdentityService"],
            serviceId: "svc_3c7addc7a558968f",
            body: {
                text: "{\n  create(request: CreateRequest) -> Identity\n  delete(request: DeleteRequest) -> bool\n}",
                span: { start: 46, end: 135 },
            },
            span: { start: 22, end: 136 },
        },
    ],
});

/** Mutate a parsed JSON fixture and return its JSON string. */
export function mutateFixture(source: string, mutate: (value: Record<string, unknown>) => void): string {
    const value = JSON.parse(source) as Record<string, unknown>;
    mutate(value);
    return JSON.stringify(value);
}
