---
name: hermes-rpc
description: Help users plan and review Hermes RPC contracts, invocation lifecycles, transports, generation, and legacy migration without creating a parallel VOS language model.
---

# Hermes RPC

Use this skill for Hermes RPC architecture, runtime, transport, code generation, compatibility, or migration from the repository's legacy `vos-*` crates.

## Ownership

- VOS owns service syntax, named parameters, canonical request/result types, stream mode, stable service/method identity, and schema compatibility.
- Hermes owns invocation lifecycle, deadline, cancellation, metadata, trace context, structured remote errors, framing, backpressure, dispatch, and transport bindings.
- Applications own handlers, auth providers, business policy, retry budgets, and deployment topology.

Never use an HTTP path, JSON-RPC method string, or source name as a substitute for VOS `MethodId`.

## Current status

Hermes has no `vos-core` mirror crate and does not vendor a local copy of the VOS language standard. Contract types come directly from upstream `vos-types`: `ServiceContract` / `ServiceMethodContract` with `methodId`, `name`, `requestType`, `responseType`, `stream`, and `operationId`. The `vos-ast`, `vos-for-rust`, `vos-faker`, `vos-openapi`, and `vos-cli` projects are frozen legacy material, removed from the workspace — do not extend them or present them as the current Hermes API.

`hermes-runtime` owns the RPC domain: artifact loading, registry, routing, dispatch. Service and operation identity (`svc_`, `op_`) is derived upstream and only mirrored, then recomputed and verified here.

Hermes does not provide an official client runtime. Clients are plain source generated ahead of time from `ServiceContract` and wired to the target project's own transport. The server side is an N-API multi-protocol host; Web support is limited to the `hermes-wasm` binding. Do not invent a local method AST, a second identity scheme, or a bundled client runtime to bypass those boundaries. Oak `service` body → method-level source projection is an upstream gap; do not fill it by parsing source text in this repository.

## Review flow

1. Classify each requirement as VOS contract, Hermes runtime, transport binding, or application policy.
2. Check that logical identity survives transport changes.
3. Require bounded resources, cancellation, deadline, structured error, and stream lifecycle behavior where applicable.
4. Call out unsupported or future behavior explicitly instead of prompt-faking a completed product surface.
