# Architecture

## Overview

Widevin is a small TypeScript library that implements the Devin/Cascade
programmatic access protocol: OAuth (PKCE) login, model discovery, and
streaming chat over Connect-RPC/protobuf. Runtime behavior is deliberately
narrow — no agent loop, CLI/TUI, proxy server, MCP tooling, retries, or
multi-provider abstraction. See [PRODUCT.md](./PRODUCT.md) for scope and
non-goals.

## Design Principles

- Prefer pure functions over classes and shared mutable state. The three
  error types (`DevinAuthError`, `DevinApiError`, `DevinProtocolError`) are
  the only classes in the package, subclassing `Error` for `instanceof`
  narrowing and structured fields (`status`, `body`, `cause`).
- Keep public exports deliberate and minimal (see `src/index.ts`).
- Make invalid states difficult to represent with TypeScript types
  (`exactOptionalPropertyTypes` and `noUncheckedIndexedAccess` are enabled).
- Separate core logic from I/O: network calls (`fetch`), the OAuth loopback
  server, and file-based token storage are isolated behind injectable
  interfaces (`FetchLike`, `TokenStore`, `openBrowser`).
- Test behavior through public APIs where practical.
- Use open source libraries when they solve a real problem better than
  custom code — the only runtime dependency is `@bufbuild/protobuf` for
  Connect/protobuf encoding.

## Project Layout

```text
.
├── docs/
│   ├── ARCHITECTURE.md
│   └── PRODUCT.md
├── examples/
│   └── hello-world.ts       # manual smoke test (login, list models, stream chat)
├── src/
│   ├── index.ts              # public entry point / export surface
│   └── devin/
│       ├── auth.ts           # PKCE + OAuth loopback callback server
│       ├── chat.ts           # chat request building + streaming response parsing
│       ├── connect.ts        # Connect-RPC frame encode/decode (gzip)
│       ├── constants.ts      # URLs, paths, versions, stop patterns
│       ├── errors.ts         # DevinAuthError / DevinApiError / DevinProtocolError
│       ├── json.ts           # lenient JSON parsing for streaming tool-call args
│       ├── models.ts         # model discovery + normalization
│       ├── proto.ts          # thin wrapper over generated protobuf schemas
│       ├── proto/            # vendored .proto sources + generated TS (see NOTICE)
│       ├── provider.ts       # createDevinProvider() facade
│       ├── token.ts          # TokenStore implementations + token normalization
│       ├── types.ts          # public types
│       └── uuid.ts           # deterministic UUID derivation (sha256-based)
├── test/
│   ├── helpers.ts
│   └── *.test.ts
├── package.json
├── tsconfig.json
├── tsconfig.build.json
└── README.md
```

## Source Organization

`src/index.ts` is the sole public entry point. It re-exports:

- `createDevinProvider` (the primary facade — login, token management,
  model discovery, chat streaming)
- Lower-level building blocks (`loginDevin`, `createPkcePair`,
  `buildDevinAuthUrl`, `exchangeDevinCliToken`, `listDevinModels`,
  `streamDevinChat`) for callers who want to compose the flow themselves
- `createMemoryTokenStore` / `createFileTokenStore`
- Error classes and public types

Everything under `src/devin/` is an implementation module; only what
`src/index.ts` re-exports is considered public API. Vendored protobuf
sources and generated code under `src/devin/proto/` are internal
implementation detail (see [NOTICE](../NOTICE)) and are never re-exported.

## API Style

Public APIs are:

- Function-based (`createDevinProvider`, `loginDevin`, `listDevinModels`,
  `streamDevinChat`, the two token store factories).
- Immutable in inputs and outputs — request/response shapes are built with
  spreads rather than mutated in place, and `DevinModel`/`DevinMessage`/
  `DevinStreamEvent` are read-only unions.
- Explicit about errors — `DevinAuthError`, `DevinApiError` (carries HTTP
  `status` and response `body`), and `DevinProtocolError` distinguish OAuth,
  HTTP, and wire-protocol failures.
- Side effects (fetch, browser launch, filesystem token storage, the OAuth
  loopback HTTP server) are all caller-injectable rather than hardcoded,
  which also keeps the library tree-shakeable and testable without network
  access.

`streamDevinChat` returns an `AsyncIterable<DevinStreamEvent>` rather than a
single value, since chat responses are inherently streamed (text/thinking
deltas, tool-call deltas, usage, and a final `done` event).

## Build Outputs

The package publishes compiled JavaScript and TypeScript declarations via
`tsconfig.build.json` (`rootDir: src`, `outDir: dist`, declarations +
source maps enabled). `package.json` already defines:

```json
{
  "type": "module",
  "exports": {
    ".": {
      "types": "./dist/index.d.ts",
      "default": "./dist/index.js"
    }
  },
  "files": ["dist", "README.md", "NOTICE"]
}
```

ESM-only, matching the `node >=20` engine floor. CommonJS consumption is not
supported and is not planned (see Non-Goals in PRODUCT.md).

## Testing Strategy

Followed test-driven development for implementation work:

1. Write a failing test that describes the desired behavior.
2. Implement the smallest change that passes the test.
3. Refactor while keeping tests green.

Current coverage (`vitest run`, `test/*.test.ts`) exercises the pure,
protocol-level logic directly:

- `auth.test.ts` — PKCE challenge derivation, OAuth URL construction, token
  exchange request shape, callback validation.
- `connect.test.ts` — Connect-RPC frame encode/decode across split chunks,
  gzip trailer decoding, oversized-frame rejection.
- `models.test.ts` — model normalization (dedup, disabled/blank filtering,
  reasoning detection) and non-200 error handling.
- `chat.test.ts` — end-to-end streamed chat request/response shape
  (compressed request, delta/tool-call/usage/done events) and prompt
  building for user/assistant/tool messages.

Network access is stubbed via an injected `fetch` (`FetchLike`); no test
hits a real Devin/Cascade endpoint. The OAuth loopback HTTP server
(`waitForOAuthCallback` in `auth.ts`) and the `createDevinProvider` facade
wiring are exercised only indirectly (through the exported pieces they
compose) rather than with dedicated integration tests.

## Protocol Quirks

- **Claude models require a non-empty system prompt for tool use.** When
  `model` targets a Claude series model and `tools` is passed,
  `systemPrompt` is effectively mandatory — Claude's tool-use path rejects
  or degrades tool calls without one. `buildChatRequest` (`chat.ts`) does
  not currently enforce this at the type or runtime level (`systemPrompt`
  stays optional in `DevinChatRequest` since it's only required for a
  subset of models/requests); callers targeting Claude models with tools
  must supply `systemPrompt` themselves. See README's Tool Calls section
  and the `DevinChatRequest.systemPrompt` JSDoc in `types.ts`.

## Dependency Policy

Prefer well-maintained open source packages for established problems such
as schema validation, parsing, formatting, or protocol handling. Before
adding a dependency, check:

- Maintenance activity.
- License compatibility.
- Bundle and install cost.
- Whether it belongs in `dependencies` or `devDependencies`.
- Whether the same result can be achieved safely with platform APIs.

Current runtime dependency: `@bufbuild/protobuf` (Connect/protobuf
encoding — no viable platform-API substitute). `@bufbuild/buf` and
`@bufbuild/protoc-gen-es` are dev-only, used by `pnpm run gen:proto` to
regenerate `src/devin/proto/generated/` from the vendored `.proto` sources.

## Publishing Architecture

Status before npm publication:

- [x] Reproducible build command (`pnpm run build`).
- [x] Explicit package exports (ESM, typed).
- [x] Generated declaration files.
- [x] Tests and type checks pass locally (`pnpm test`, `pnpm run typecheck`).
- [ ] License file and `package.json` `license` field (currently unset).
- [ ] Prepublish/release check wiring build + typecheck + test into one
      script (currently run separately).
- [ ] Confirm no accidental source, test fixture, or local configuration
      files are included — `files` is already scoped to `dist`, `README.md`,
      `NOTICE`, which is correct as-is.

## Open Questions

- Whether to vendor-refresh `src/devin/proto/generated/` on a schedule, or
  only on demand when the upstream `oh-my-pi` proto sources change.
- Release automation (versioning, changelog, publish workflow) — not yet
  decided; no CI/release pipeline exists in this repo.
