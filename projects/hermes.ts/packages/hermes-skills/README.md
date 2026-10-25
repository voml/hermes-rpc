# `@yydb/hermes-skills`

Agent Skills for designing and reviewing Hermes RPC work without recreating the VOS language or coupling service identity to a transport.

Hermes consumes VOS `ServiceContract` artifacts and performs AOT generation plus protocol hosting; it does not provide an official client runtime or parse VOS source. The skill helps classify work, review boundaries, and plan runtime behavior, and it does not claim unfinished transports or generators already exist.

## Install

```bash
npx skills add @yydb/hermes-skills --skill hermes-rpc -y
```

Preview or install globally:

```bash
npx skills add @yydb/hermes-skills --list
npx skills add @yydb/hermes-skills --skill hermes-rpc -y -g
```

## Start with a prompt

```text
Review this RPC requirement and separate what belongs to the VOS ServiceContract,
the Hermes invocation model, and the transport binding.

Design cancellation, deadlines, metadata, and structured errors for this unary call
without using an HTTP path as the method identity.

Audit this repository for code that still treats the legacy vos-* crates as the
current VOS parser or service model.
```

The installer requires Node.js 18+. This is a docs-only skill package.
