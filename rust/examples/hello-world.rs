//! Manual smoke test for the Rust crate.
//!
//! Run:
//!   cargo run --manifest-path rust/Cargo.toml --example hello-world
//!   cargo run --manifest-path rust/Cargo.toml --example hello-world -- "Say hello."

use futures_util::StreamExt;
use std::{process::Command, sync::Arc};
use widevin::{
    DevinChatRequest, DevinContentPart, DevinError, DevinMessage, DevinProviderOptions,
    DevinStopReason, DevinStreamEvent, create_devin_provider, create_file_token_store,
};

const TOKEN_PATH: &str = ".devin-token";

fn open_browser(url: &str) {
    eprintln!("Open this URL to sign in to Devin: {url}");
    let result = if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/c", "start", "", url]).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };
    if result.is_err() {
        eprintln!("Could not launch a browser automatically; open the URL above manually.");
    }
}

#[tokio::main]
async fn main() -> Result<(), DevinError> {
    let message = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let message = if message.is_empty() {
        "Say hello in one short sentence.".into()
    } else {
        message
    };
    let token_store = create_file_token_store(TOKEN_PATH);
    let provider = create_devin_provider(DevinProviderOptions {
        token_store: Some(token_store.clone()),
        open_browser: Some(Arc::new(|url| {
            Box::pin(async move {
                open_browser(&url);
                Ok(())
            })
        })),
        ..Default::default()
    });

    if let Ok(token) = std::env::var("DEVIN_TOKEN") {
        eprintln!("Using DEVIN_TOKEN from the environment.");
        provider.set_token(token).await?;
    } else if token_store.get().await?.is_some() {
        eprintln!("Using cached token from {TOKEN_PATH}");
    } else {
        eprintln!("No cached token found, starting Devin login...");
        provider.login().await?;
        eprintln!("Login complete. Token cached at {TOKEN_PATH}");
    }

    eprintln!("Fetching available models...");
    let model = provider
        .list_models()
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| DevinError::protocol("Devin returned no available models"))?;
    eprintln!("Using model {} ({})", model.id, model.name);
    eprintln!("Sending: {message}");
    eprintln!("---");

    let mut events = provider.stream_chat(DevinChatRequest {
        model: model.id,
        system_prompt: vec!["You are concise.".into()],
        messages: vec![DevinMessage::User {
            content: vec![DevinContentPart::Text { text: message }],
        }],
        ..Default::default()
    });
    let mut saw_text = false;
    while let Some(event) = events.next().await {
        match event? {
            DevinStreamEvent::TextDelta { delta } => {
                saw_text = true;
                print!("{delta}");
            }
            DevinStreamEvent::Usage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_write_tokens,
            } => eprintln!(
                "\n[usage] input={input_tokens} output={output_tokens} \
                 cacheRead={cache_read_tokens} cacheWrite={cache_write_tokens}"
            ),
            DevinStreamEvent::Done { reason } => {
                if saw_text {
                    println!();
                }
                let reason = match reason {
                    DevinStopReason::Stop => "stop",
                    DevinStopReason::Length => "length",
                    DevinStopReason::ToolUse => "toolUse",
                };
                eprintln!("[done] reason={reason}");
            }
            _ => {}
        }
    }
    Ok(())
}
