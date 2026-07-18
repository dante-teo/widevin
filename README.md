# widevin

Lightweight Node 20+ ESM TypeScript library and Rust 1.85+ crate for
Devin/Cascade programmatic access.

It provides OAuth login, caller-controlled token storage, model discovery, and streaming chat with tool-call events. It intentionally does not include an agent loop, CLI/TUI, proxy server, MCP tooling, retries, or a multi-provider framework.

## Install

TypeScript:

```sh
pnpm add widevin
```

Rust:

```sh
cargo add widevin
```

The TypeScript and Rust packages share version `0.1.4` and expose equivalent
OAuth, token-store, model-discovery, streaming chat, injection, and structured
error behavior. Rust users can start with the
[`rust/README.md`](./rust/README.md).

## Compliance

Only use Devin/Cascade programmatic access when permitted by your organization and applicable terms. This package sends requests compatible with the Devin/Cascade protocol but does not grant permission to use that service.

## Login

```ts
import { createDevinProvider, createFileTokenStore } from "widevin";

const devin = createDevinProvider({
  tokenStore: createFileTokenStore("./.devin-token"),
  openBrowser: async (url) => {
    console.log(`Open this URL to sign in: ${url}`);
  }
});

await devin.login();
```

`openBrowser` is intentionally injected. The library never launches a browser by itself.

## Existing Token

```ts
import { createDevinProvider } from "widevin";

const devin = createDevinProvider();

await devin.setToken(process.env.DEVIN_TOKEN ?? "");
```

Store raw tokens. The `devin-session-token$` prefix is applied only at request
boundaries. An empty token is treated as absent rather than being prefixed.

## List Models

```ts
const models = await devin.listModels();

for (const model of models) {
  console.log(model.id, model.name, model.reasoning);
}
```

## Stream Chat

```ts
const events = devin.streamChat({
  model: "claude-sonnet-4",
  systemPrompt: ["You are concise."],
  messages: [{ role: "user", content: "Summarize this repo." }]
});

for await (const event of events) {
  if (event.type === "text_delta") process.stdout.write(event.delta);
}
```

> **Claude models require a system prompt for tool use.** When `model`
> targets a Claude series model (e.g. `claude-sonnet-4`, `claude-opus-4`)
> and `tools` is non-empty, `systemPrompt` is mandatory — Claude's tool-use
> path requires a non-empty system prompt, and omitting it produces
> degraded or rejected tool-call behavior upstream. Non-Claude models are
> unaffected; `systemPrompt` remains optional for plain text chat.

## Tool Calls

```ts
for await (const event of devin.streamChat({
  model: "claude-sonnet-4",
  messages: [{ role: "user", content: "Search for TypeScript files." }],
  tools: [
    {
      name: "search",
      description: "Search project files.",
      inputSchema: {
        type: "object",
        properties: { query: { type: "string" } },
        required: ["query"]
      }
    }
  ]
})) {
  if (event.type === "toolcall_end") {
    console.log(event.id, event.name, event.arguments);
  }
}
```

Every `toolcall_delta` contains the raw `delta` suffix. Its optional
`arguments` field is an opportunistic parsed snapshot and is throttled during
streaming, so it may be absent even when the latest accumulated buffer is
valid JSON. Use `toolcall_end.arguments` as the authoritative parse of the
full accumulated buffer.

Pass tool results back as history:

```ts
await devin.streamChat({
  model: "claude-sonnet-4",
  messages: [
    { role: "user", content: "Search for TypeScript files." },
    {
      role: "assistant",
      responseId: "assistant-message-id",
      content: [{ type: "toolCall", id: "call-1", name: "search", arguments: { query: "*.ts" } }]
    },
    { role: "tool", toolCallId: "call-1", content: "src/index.ts" }
  ]
});
```

## Development

```sh
pnpm install
pnpm run gen:proto
pnpm run check:versions
pnpm run check

cargo fmt --manifest-path rust/Cargo.toml --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path rust/Cargo.toml --all-features
cargo doc --manifest-path rust/Cargo.toml --no-deps
cargo package --manifest-path rust/Cargo.toml
```

CI runs the Rust tests on Linux and Windows, and performs the full Rust 1.85
format, Clippy, test, documentation, and package checks on Linux. See
[`docs/RELEASING.md`](./docs/RELEASING.md) for shared-version and publishing
details.

## Manual Smoke Test

See [`examples/hello-world.ts`](./examples/hello-world.ts) for a runnable script that logs in via the browser (or reuses a cached/env token), lists models, and streams a chat response. Run it with:

```sh
pnpm run example:hello
pnpm run example:hello -- "Explain what this library does in one sentence."
```

The matching Rust smoke test is
[`rust/examples/hello-world.rs`](./rust/examples/hello-world.rs):

```sh
cargo run --manifest-path rust/Cargo.toml --example hello-world
cargo run --manifest-path rust/Cargo.toml --example hello-world -- \
  "Explain what this library does in one sentence."
```

The protobuf source files under `src/devin/proto/source` are vendored from `can1357/oh-my-pi` for protocol compatibility. Generated TypeScript is committed so consumers do not need protoc or Buf at install time.
Rust uses a committed minimal Prost module containing only the fields used at
runtime, so Rust consumers also need no protobuf compiler.
Both published packages include their applicable `NOTICE` file with protocol
provenance.
