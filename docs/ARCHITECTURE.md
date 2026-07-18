# Architecture

## Overview

Widevin is a TypeScript library and Rust crate that implement the
Devin/Cascade programmatic access protocol: OAuth (PKCE) login, model
discovery, and streaming chat over Connect-RPC/protobuf. Their public names
follow each language's conventions while their request defaults, wire
encoding, normalization, event ordering, and error categories stay aligned.
Runtime behavior is deliberately narrow — no agent loop, CLI/TUI, proxy
server, MCP tooling, retries, or multi-provider abstraction. See
[PRODUCT.md](./PRODUCT.md) for scope and non-goals.

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
- Keep TypeScript and Rust protocol fixtures cross-decodable.
- Use open source libraries when they solve a real problem better than
  custom code. TypeScript uses `@bufbuild/protobuf`; Rust uses established
  Tokio/HTTP, wire-format, compression, serialization, crypto, and stream
  crates listed in `rust/Cargo.toml`.

## Project Layout

```text
.
├── docs/
│   ├── ARCHITECTURE.md
│   ├── PRODUCT.md
│   └── RELEASING.md
├── examples/
│   └── hello-world.ts       # manual smoke test (login, list models, stream chat)
├── rust/
│   ├── examples/hello-world.rs
│   ├── fixtures/protobuf/    # cross-language wire fixtures
│   ├── src/                  # functional Rust implementation + minimal Prost schema
│   ├── tests/                # Rust parity and failure-path tests
│   ├── NOTICE
│   └── Cargo.toml
├── src/
│   ├── index.ts              # public entry point / export surface
│   └── devin/
│       ├── auth.ts           # PKCE + OAuth loopback callback server
│       ├── chat.ts           # chat request building + streaming response parsing
│       ├── connect.ts        # Connect-RPC frame encode/decode (gzip)
│       ├── constants.ts      # URLs, paths, versions, stop patterns
│       ├── errors.ts         # DevinAuthError / DevinApiError / DevinProtocolError
│       ├── json.ts           # JSON parsing + throttling for streaming tool-call args
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

`src/index.ts` is the sole TypeScript public entry point. It re-exports:

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

`rust/src/lib.rs` is the Rust public entry point. It exports snake-case
equivalents and immutable structs/enums. Async iterables become
`Stream<Item = Result<DevinStreamEvent, DevinError>>`; fetch, browser, UUID,
and token-store boundaries remain closure- or trait-injectable. The committed
minimal Prost module means crates.io consumers do not need `protoc`.

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

Tool-call argument accumulation is request-local. Each tool-call ID has one
immutable state value containing its name, accumulated JSON, and last parse
attempt length. A pure transition function computes the next state and raw
delta; the async generator owns the `Map` and network/event side effects.

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
  "files": ["dist", "README.md", "NOTICE", "LICENSE"]
}
```

ESM-only, matching the `node >=20` engine floor. CommonJS consumption is not
supported and is not planned (see Non-Goals in PRODUCT.md).

The Rust crate uses edition 2024 with MSRV 1.85 and publishes from `rust/`.
It uses Tokio, Reqwest/Rustls, Prost, Flate2, Serde JSON, SHA-2, Base64,
UUID, IndexMap, Async Stream, and Thiserror. Its explicit package include list
ships the minimal schema, cross-language fixtures, README, MIT license, and
Rust-specific protocol `NOTICE`.

## Testing Strategy

Followed test-driven development for implementation work:

1. Write a failing test that describes the desired behavior.
2. Implement the smallest change that passes the test.
3. Refactor while keeping tests green.

TypeScript coverage (`vitest run`, `test/*.test.ts`) and Rust coverage
(`cargo test --manifest-path rust/Cargo.toml`) exercise the pure,
protocol-level logic directly:

- `auth.test.ts` — PKCE challenge derivation, OAuth URL construction, token
  exchange request shape, callback validation.
- `connect.test.ts` — Connect-RPC frame encode/decode across split chunks,
  gzip trailer decoding, oversized-frame rejection.
- `models.test.ts` — model normalization (dedup, disabled/blank filtering,
  reasoning detection) and non-200 error handling.
- `chat.test.ts` — end-to-end streamed chat request/response shape
  (compressed request, cumulative and incremental tool arguments,
  per-tool-call parse throttling, usage/done events) and prompt building for
  user/assistant/tool messages.
- `json.test.ts` — strict JSON fallback semantics and the streaming parse
  throttle's first-attempt and 256-character growth guarantees.
- `release-metadata.test.ts` — branch/tag version-check behavior and Rust
  protocol-notice packaging metadata.
- `version-bump.test.ts` — synchronized npm/Cargo/README bumps, prereleases,
  explicit versions, invalid input, pre-existing drift, Git cleanliness,
  LF/CRLF preservation, filesystem-safe script paths, commit/tag rollback,
  annotated release tags, and no-push behavior.

Network access is stubbed via an injected `fetch` (`FetchLike`); no test
hits a real Devin/Cascade endpoint. Rust integration tests additionally
exercise OAuth loopback cleanup, file permissions, provider wiring,
transport/store failures, missing and malformed responses, partial frames,
empty-token handling, punctuated reasoning labels, and both directions of
TypeScript/Prost fixture decoding. CI runs Rust tests on Linux and Windows;
the Linux job pins Rust 1.85 and also enforces formatting, Clippy, docs, and
crate packaging.

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
- **Mid-stream tool-argument parsing is throttled per tool-call ID.** The
  first non-empty buffer is attempted immediately; another attempt requires
  at least 256 characters of growth. Every raw `toolcall_delta` is still
  emitted, but its optional `arguments` snapshot can lag the buffer. The
  unconditional `toolcall_end` parse is authoritative. This bounds repeated
  full-buffer parsing to linear rather than quadratic total work.
- **Empty credentials are absent credentials.** Both implementations leave an
  empty token unprefixed, while non-empty raw tokens receive the
  `devin-session-token$` prefix at request boundaries.
- **Model reasoning labels honor `No Thinking` as a bounded phrase.**
  Punctuation such as `Claude (No Thinking)` still disables reasoning, while
  embedded text that does not satisfy the word boundaries does not.
- **The OAuth callback is a bounded loopback HTTP exchange.** Rust accepts
  request headers split across TCP reads and caps the accumulated header at
  16 KiB before validation.

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

Both packages use one semantic version and one `vX.Y.Z` tag. Branch and
pull-request CI verifies that the npm and Cargo manifest versions agree; tag
builds additionally require the tag version to match. Publication order is
crates.io, then npm, and each step skips a version already present in its
registry.

Rust `0.1.4` was published manually so crates.io trusted publishing can be
configured. Bind repository
`dante-teo/widevin`, workflow `publish.yml`, and environment `release` as the
trusted publisher. Future version changes use `pnpm bump`, which updates both
manifests, the Cargo lockfile, and current-version README references without
publishing. It requires a clean working tree, creates a local version commit
and annotated tag, and never pushes them. No release workflow is run by this
implementation work.
See [RELEASING.md](./RELEASING.md) for the operational checklist.

## Open Questions

- Whether to vendor-refresh `src/devin/proto/generated/` on a schedule, or
  only on demand when the upstream `oh-my-pi` proto sources change.
- Whether protobuf fixture regeneration should stay manual or become a
  checked code-generation task.
