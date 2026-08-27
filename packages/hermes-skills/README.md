# `@game-gpt/hermes-skills`

Agent Skills for designing and reviewing Hermes RPC work without recreating the VOS language or coupling service identity to a transport.

Hermes is currently being rebuilt around VOS `ServiceContract`. The skill is honest about that status: it can help classify work, review boundaries, and plan runtime behavior, but it does not claim that unfinished Hermes transports or generators already exist.

## Install

```bash
npx skills add @game-gpt/hermes-skills --skill hermes-rpc -y
```

Preview or install globally:

```bash
npx skills add @game-gpt/hermes-skills --list
npx skills add @game-gpt/hermes-skills --skill hermes-rpc -y -g
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
