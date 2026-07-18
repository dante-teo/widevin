use crate::{
    DevinError, FetchLike, TokenStoreRef,
    constants::{DEVIN_APP_BASE_URL, DEVIN_AUTH_BASE_URL, DEVIN_DEFAULT_BASE_URL},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{future::Future, pin::Pin, sync::Arc};

pub type OpenBrowserFuture = Pin<Box<dyn Future<Output = Result<(), DevinError>> + Send + 'static>>;
pub type OpenBrowser = Arc<dyn Fn(String) -> OpenBrowserFuture + Send + Sync + 'static>;
pub type UuidGenerator = Arc<dyn Fn() -> String + Send + Sync + 'static>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DevinInput {
    Text,
    Image,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevinModel {
    pub id: String,
    pub name: String,
    pub provider: &'static str,
    pub base_url: String,
    pub input: Vec<DevinInput>,
    pub supports_tools: bool,
    pub reasoning: bool,
    pub context_window: i32,
    pub max_tokens: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DevinContentPart {
    Text { text: String },
    Image { data: String, mime_type: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum DevinAssistantContentPart {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
        thinking_signature: Option<String>,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: Value,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum DevinMessage {
    User {
        content: Vec<DevinContentPart>,
    },
    Developer {
        content: Vec<DevinContentPart>,
    },
    Assistant {
        content: Vec<DevinAssistantContentPart>,
        response_id: Option<String>,
    },
    Tool {
        tool_call_id: String,
        content: Vec<DevinContentPart>,
        is_error: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DevinTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub strict: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DevinChatRequest {
    pub model: String,
    pub messages: Vec<DevinMessage>,
    pub system_prompt: Vec<String>,
    pub tools: Vec<DevinTool>,
    pub conversation_id: Option<String>,
    pub session_id: Option<String>,
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stop_sequences: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DevinStopReason {
    Stop,
    Length,
    ToolUse,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DevinStreamEvent {
    TextDelta {
        delta: String,
    },
    ThinkingDelta {
        delta: String,
        signature: Option<String>,
    },
    ToolCallStart {
        id: String,
        name: String,
    },
    ToolCallDelta {
        id: String,
        delta: String,
        arguments: Option<Value>,
    },
    ToolCallEnd {
        id: String,
        name: String,
        arguments: Value,
    },
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_write_tokens: u64,
    },
    Done {
        reason: DevinStopReason,
    },
}

#[derive(Clone)]
pub struct DevinProviderOptions {
    pub token_store: Option<TokenStoreRef>,
    pub fetch: Option<FetchLike>,
    pub base_url: String,
    pub auth_base_url: String,
    pub app_base_url: String,
    pub open_browser: Option<OpenBrowser>,
    pub uuid: Option<UuidGenerator>,
}

impl Default for DevinProviderOptions {
    fn default() -> Self {
        Self {
            token_store: None,
            fetch: None,
            base_url: DEVIN_DEFAULT_BASE_URL.into(),
            auth_base_url: DEVIN_AUTH_BASE_URL.into(),
            app_base_url: DEVIN_APP_BASE_URL.into(),
            open_browser: None,
            uuid: None,
        }
    }
}

impl std::fmt::Debug for DevinProviderOptions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DevinProviderOptions")
            .field(
                "token_store",
                &self.token_store.as_ref().map(|_| "<token store>"),
            )
            .field("fetch", &self.fetch.as_ref().map(|_| "<fetch>"))
            .field("base_url", &self.base_url)
            .field("auth_base_url", &self.auth_base_url)
            .field("app_base_url", &self.app_base_url)
            .field(
                "open_browser",
                &self.open_browser.as_ref().map(|_| "<browser>"),
            )
            .field("uuid", &self.uuid.as_ref().map(|_| "<uuid generator>"))
            .finish()
    }
}
