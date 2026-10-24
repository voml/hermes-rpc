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

`vos-core` is this repository's host for the VOS language standard: schema modeling plus serde mirrors of the upstream service and operation projection artifacts. It is not legacy. The `vos-ast`, `vos-for-rust`, `vos-faker`, `vos-openapi`, and `vos-cli` projects are frozen legacy material — do not extend them or present them as the current Hermes API.

`hermes-runtime` owns the RPC domain: artifact loading, registry, routing, dispatch. Service and operation identity (`svc_`, `op_`) is derived upstream and only mirrored here.

The upstream service projection stops at syntax projection, so it carries no method descriptor yet. Until VOS delivers method-level projection, limit implementation advice to operation-level dispatch and transport-neutral runtime primitives. Do not invent a local method AST, method identity scheme, or generation model to bypass that dependency.

## Review flow

1. Classify each requirement as VOS contract, Hermes runtime, transport binding, or application policy.
2. Check that logical identity survives transport changes.
3. Require bounded resources, cancellation, deadline, structured error, and stream lifecycle behavior where applicable.
4. Call out unsupported or future behavior explicitly instead of prompt-faking a completed product surface.
