# widevin for Rust

Rust 1.85+ bindings for Devin/Cascade OAuth login, caller-controlled token
storage, model discovery, and streaming chat/tool events. This crate mirrors
the behavior of the TypeScript `widevin` package and uses Reqwest with Rustls.

Only use Devin/Cascade programmatic access when permitted by your organization
and applicable terms.

## Install

```toml
[dependencies]
widevin = "0.2.0"
futures-util = "0.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quick start

```rust,no_run
use futures_util::StreamExt;
use widevin::{
    DevinChatRequest, DevinContentPart, DevinMessage, DevinStreamEvent,
    create_devin_provider,
};

#[tokio::main]
async fn main() -> Result<(), widevin::DevinError> {
    let provider = create_devin_provider(Default::default());
    provider.set_token(std::env::var("DEVIN_TOKEN").unwrap_or_default()).await?;
    let model = provider.list_models().await?.into_iter().next()
        .ok_or_else(|| widevin::DevinError::protocol("Devin returned no models"))?;
    let mut events = provider.stream_chat(DevinChatRequest {
        model: model.id,
        system_prompt: vec!["You are concise.".into()],
        messages: vec![DevinMessage::User {
            content: vec![DevinContentPart::Text {
                text: "Say hello in one sentence.".into(),
            }],
        }],
        ..Default::default()
    });
    while let Some(event) = events.next().await {
        if let DevinStreamEvent::TextDelta { delta } = event? {
            print!("{delta}");
        }
    }
    Ok(())
}
```

Store raw tokens. The `devin-session-token$` prefix is added only at request
boundaries; an empty token is treated as absent. File token stores create
private `0600` files on Unix. `ToolCallDelta.arguments` may contain a
throttled parsed JSON snapshot; the final `ToolCallEnd.arguments` value is
authoritative.

Run the full smoke-test example from the repository:

```sh
cargo run --manifest-path rust/Cargo.toml --example hello-world
cargo run --manifest-path rust/Cargo.toml --example hello-world -- \
  "Explain what this library does in one sentence."
```

The example uses `DEVIN_TOKEN`, `.devin-token`, or the OAuth callback flow in
that order. Delete `.devin-token` to remove the cached local token; this does
not revoke the credential server-side.
