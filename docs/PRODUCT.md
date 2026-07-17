# Product

## Purpose

Widevin is a TypeScript library for programmatic access to Devin/Cascade:
OAuth login, caller-controlled token storage, model discovery, and streaming
chat with tool-call events. It is distributed through npm and consumed from
modern TypeScript and JavaScript (Node.js ≥20, ESM) projects.

## Compliance

Only use Devin/Cascade programmatic access when permitted by your
organization and applicable terms. Widevin sends requests compatible with
the Devin/Cascade protocol but does not itself grant permission to use that
service (see [README.md](../README.md)).

## Product Goals

- Provide a small, reliable public API with predictable behavior:
  `createDevinProvider`, plus lower-level `loginDevin`, `listDevinModels`,
  `streamDevinChat` for callers who want to compose the flow themselves.
- Favor pure functions and immutable data over hidden state. Long-lived
  mutable state exists only behind the two `TokenStore` implementations;
  request-local protocol state is scoped to one async operation and never
  escapes it.
- Ship first-class TypeScript types (`DevinModel`, `DevinMessage`,
  `DevinStreamEvent`, `DevinChatRequest`, etc.), generated under `strict`
  mode with `exactOptionalPropertyTypes` and `noUncheckedIndexedAccess`.
- Work cleanly in Node.js projects (ESM only; no CommonJS or browser target
  is currently supported).
- Keep installation and runtime dependency cost low — one runtime
  dependency (`@bufbuild/protobuf`).
- Maintain documentation that makes the package easy to adopt without
  reading the source (README usage examples cover login, existing-token
  use, model listing, chat streaming, and tool calls).

## Non-Goals

- Not a framework: no agent loop, CLI/TUI, proxy server, or MCP tooling.
- No retries or backoff — callers own their own retry policy via the
  injectable `fetch`/`AbortSignal`.
- No multi-provider abstraction — this package speaks only the
  Devin/Cascade protocol.
- Do not expose unstable internals as public API — only what
  `src/index.ts` re-exports is supported; `src/devin/proto/**` (vendored
  protobuf) is internal.
- Do not add dependencies unless they clearly improve correctness,
  interoperability, or maintainability.
- Do not optimize for every runtime (e.g. browser, edge, CommonJS) until a
  real consumer need exists.

## Target Users

- TypeScript/JavaScript developers building a Devin/Cascade-compatible
  chat client or agent runtime who want a typed, dependency-light provider
  implementation rather than reimplementing the Connect-RPC protocol.
- Maintainers who need tests, build scripts, and release metadata suitable
  for npm publishing.

## Package Expectations

Delivered:

- `package.json` with npm metadata (name, version, description, engines,
  exports, files).
- TypeScript source under `src/`, generated JavaScript and declaration
  files under `dist/` via `pnpm run build`.
- Unit tests for public/protocol-level behavior (`test/*.test.ts`,
  `vitest run`).
- README usage examples (login, existing token, list models, stream chat,
  tool calls, tool-result history).
- `NOTICE` documenting vendored protobuf sources and prior-art reference.

Outstanding before publication:

- License information (`LICENSE` file + `package.json` `license` field are
  not yet set).
- Changelog / release notes (no versions have been published yet).

## Quality Bar

Every public feature has:

- A clear use case (see README sections: Login, Existing Token, List
  Models, Stream Chat, Tool Calls).
- A small, documented API surface (`src/index.ts` is the only public
  entry point).
- Tests that describe expected behavior for the protocol-level logic
  (PKCE, Connect-RPC framing, model normalization, chat streaming/prompt
  building).
- Type definitions that match runtime behavior (`strict` TypeScript,
  `exactOptionalPropertyTypes`).
- Intentional error behavior: `DevinAuthError` (OAuth), `DevinApiError`
  (HTTP failures, carries `status`/`body`), `DevinProtocolError` (wire
  format failures) are distinct, documented types.
- Documented protocol quirks that affect callers: `systemPrompt` is
  mandatory (not merely recommended) when `model` is a Claude series model
  and `tools` is passed, since Claude's tool-use path requires a non-empty
  system prompt; streamed `toolcall_delta.arguments` is an optional,
  throttled snapshot while `toolcall_end.arguments` is authoritative (see
  README Tool Calls section).

## Release Readiness

Before publishing to npm:

- [ ] Confirm the `widevin` package name is available on npm, or scope it.
- [x] The npm `files` field only includes publishable artifacts
      (`dist`, `README.md`, `NOTICE`).
- [x] Build, typecheck, and test commands pass locally (`pnpm run build`,
      `pnpm run typecheck`, `pnpm test`). No lint command is configured yet.
- [x] The README contains install and usage instructions.
- [x] The package exports are ESM-only and match the stated `node >=20`
      engine floor.
- [ ] Add a `license` field to `package.json` and a `LICENSE` file.
- [ ] Decide semantic versioning / release process before the first
      publish (currently `0.1.4`, unpublished).

## Open Questions

- Whether to support CommonJS consumers — not planned; revisit only if a
  real consumer requires it (Non-Goals).
- License choice and release automation (versioning, changelog, publish
  workflow) — not yet decided.
