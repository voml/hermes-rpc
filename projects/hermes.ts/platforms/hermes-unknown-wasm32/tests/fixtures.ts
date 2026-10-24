import { readFileSync } from "node:fs";

/** Hermes contract reference artifact (`vos/hermes.contract/v0`). */
export const CONTRACT_ARTIFACT = readFileSync(
    new URL("./fixtures/operations_services.contract.json", import.meta.url),
    "utf8",
);

/** Service exposed by the reference artifact. */
export const SERVICE_NAME = "IdentityService";

/** Unary method exposed by the reference artifact. */
export const CREATE_METHOD = "create";

/** Server-stream method exposed by the reference artifact. */
export const LOOKUP_METHOD = "lookup";

/** Durable method id of the unary `create` method. */
export const CREATE_METHOD_ID = "mtd_a1195cc42bc0418387f233d5db60833493a2aecab3923e75c97be3509f0aecfd";

/** Durable operation id bound to the unary `create` method. */
export const CREATE_OPERATION_ID = "op_38ecee3afe0deee18f4f4c89f7b33c390d84844a3bf0832632f2f612cd132437";

/** Mutate a parsed JSON fixture and return its JSON string. */
export function mutateFixture(source: string, mutate: (value: Record<string, unknown>) => void): string {
    const value = JSON.parse(source) as Record<string, unknown>;
    mutate(value);
    return JSON.stringify(value);
}
