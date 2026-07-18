use crate::{
    ByteStream, DevinAssistantContentPart, DevinChatRequest, DevinContentPart, DevinError,
    DevinMessage, DevinStopReason, DevinStreamEvent, DevinTool, FetchLike, FetchRequest,
    FetchResponse, UuidGenerator,
    connect::{
        decode_connect_frames, decode_proto_with_gzip_fallback, encode_connect_frame,
        read_connect_trailer_error,
    },
    constants::{
        DEVIN_AUTH_PATH, DEVIN_CHAT_PATH, DEVIN_DEFAULT_BASE_URL, DEVIN_DEFAULT_STOP_PATTERNS,
        trim_trailing_slashes,
    },
    fetch::{default_fetch, read_all},
    json::{parse_possibly_complete_json_throttled, parse_streaming_json},
    metadata, normalize_devin_session_token,
    proto::{
        CacheControlType, ChatMessagePrompt, ChatMessageRequestType, ChatMessageSource,
        ChatToolCall, ChatToolChoice, ChatToolDefinition, CompletionConfiguration,
        ConversationalPlannerMode, GetChatMessageRequest, GetChatMessageResponse,
        GetUserJwtRequest, GetUserJwtResponse, ImageData, PromptCacheOptions, StopReason,
        chat_tool_choice,
    },
};
use async_stream::try_stream;
use futures_core::Stream;
use futures_util::StreamExt;
use indexmap::IndexMap;
use prost::Message;
use serde_json::Value;
use std::{pin::Pin, sync::Arc};
use uuid::Uuid;

pub type DevinEventStream =
    Pin<Box<dyn Stream<Item = Result<DevinStreamEvent, DevinError>> + Send>>;

#[derive(Clone)]
pub struct StreamDevinChatOptions {
    pub token: Option<String>,
    pub base_url: String,
    pub fetch: Option<FetchLike>,
    pub uuid: Option<UuidGenerator>,
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

impl Default for StreamDevinChatOptions {
    fn default() -> Self {
        Self {
            token: None,
            base_url: DEVIN_DEFAULT_BASE_URL.into(),
            fetch: None,
            uuid: None,
            model: String::new(),
            messages: Vec::new(),
            system_prompt: Vec::new(),
            tools: Vec::new(),
            conversation_id: None,
            session_id: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop_sequences: Vec::new(),
        }
    }
}

impl StreamDevinChatOptions {
    pub(crate) fn from_request(
        request: DevinChatRequest,
        token: Option<String>,
        base_url: String,
        fetch: FetchLike,
        uuid: Option<UuidGenerator>,
    ) -> Self {
        Self {
            token,
            base_url,
            fetch: Some(fetch),
            uuid,
            model: request.model,
            messages: request.messages,
            system_prompt: request.system_prompt,
            tools: request.tools,
            conversation_id: request.conversation_id,
            session_id: request.session_id,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            stop_sequences: request.stop_sequences,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthMetadata {
    pub user_jwt: String,
    pub base_url: Option<String>,
}

#[derive(Clone, Debug)]
struct StreamingToolCallState {
    name: String,
    arguments_json: String,
    last_parse_attempt_length: usize,
}

/// Streams chat events after fetching user-JWT metadata.
///
/// Errors are yielded as [`DevinError`] stream items.
pub fn stream_devin_chat(options: StreamDevinChatOptions) -> DevinEventStream {
    Box::pin(try_stream! {
        let fetch = options.fetch.clone().unwrap_or_else(default_fetch);
        let base_url = trim_trailing_slashes(&options.base_url);
        let token = normalize_devin_session_token(options.token.as_deref());
        let body = open_chat_response(
            &options,
            &token,
            &base_url,
            fetch,
        )
        .await?;

        let frames = decode_connect_frames(body);
        futures_util::pin_mut!(frames);
        let mut tool_calls = IndexMap::<String, StreamingToolCallState>::new();
        let mut active_tool_call_id = None::<String>;
        let mut saw_tool_call = false;
        let mut latest_stop_reason = StopReason::Unspecified;

        while let Some(frame) = frames.next().await {
            let frame = frame?;
            if frame.end_stream {
                if let Some(error) = read_connect_trailer_error(&frame.payload) {
                    Err(DevinError::protocol(error))?;
                }
                continue;
            }
            let message = GetChatMessageResponse::decode(frame.payload.as_slice())
                .map_err(|error| DevinError::protocol(format!(
                    "invalid Devin chat protobuf response: {error}"
                )))?;
            if !message.delta_thinking.is_empty() {
                yield DevinStreamEvent::ThinkingDelta {
                    delta: message.delta_thinking,
                    signature: (!message.delta_signature.is_empty())
                        .then_some(message.delta_signature),
                };
            }
            if !message.delta_text.is_empty() {
                yield DevinStreamEvent::TextDelta {
                    delta: message.delta_text,
                };
            }
            for call in message.delta_tool_calls {
                let id = if call.id.is_empty() {
                    active_tool_call_id.clone()
                } else {
                    Some(call.id.clone())
                };
                let Some(id) = id else {
                    continue;
                };
                let previous = tool_calls.get(&id).cloned();
                let (state, delta, parsed_arguments) =
                    advance_streaming_tool_call(previous.as_ref(), &call);
                tool_calls.insert(id.clone(), state.clone());
                active_tool_call_id = Some(id.clone());
                saw_tool_call = true;
                if previous.is_none() {
                    yield DevinStreamEvent::ToolCallStart {
                        id: id.clone(),
                        name: state.name.clone(),
                    };
                }
                yield DevinStreamEvent::ToolCallDelta {
                    id,
                    delta,
                    arguments: parsed_arguments,
                };
            }
            if let Ok(reason) = StopReason::try_from(message.stop_reason) {
                if reason != StopReason::Unspecified {
                    latest_stop_reason = reason;
                }
            }
            if let Some(usage) = message.usage {
                yield DevinStreamEvent::Usage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cache_read_tokens: usage.cache_read_tokens,
                    cache_write_tokens: usage.cache_write_tokens,
                };
            }
        }
        for (id, state) in tool_calls {
            yield DevinStreamEvent::ToolCallEnd {
                id,
                name: state.name,
                arguments: parse_streaming_json(Some(&state.arguments_json)),
            };
        }
        yield DevinStreamEvent::Done {
            reason: if saw_tool_call {
                DevinStopReason::ToolUse
            } else if latest_stop_reason == StopReason::MaxTokens {
                DevinStopReason::Length
            } else {
                DevinStopReason::Stop
            },
        };
    })
}

async fn open_chat_response(
    options: &StreamDevinChatOptions,
    token: &str,
    base_url: &str,
    fetch: FetchLike,
) -> Result<ByteStream, DevinError> {
    let auth = fetch_auth_metadata(token, base_url, Arc::clone(&fetch)).await?;
    let chat_base_url = auth.base_url.unwrap_or_else(|| base_url.to_owned());
    let uuid = options
        .uuid
        .clone()
        .unwrap_or_else(|| Arc::new(|| Uuid::new_v4().to_string()));
    let request = build_chat_request(options, token, &auth.user_jwt, uuid.as_ref());
    let response = fetch(FetchRequest {
        url: format!("{chat_base_url}{DEVIN_CHAT_PATH}"),
        method: "POST".into(),
        headers: IndexMap::from([
            ("content-type".into(), "application/connect+proto".into()),
            ("connect-protocol-version".into(), "1".into()),
            ("connect-content-encoding".into(), "gzip".into()),
            ("accept-encoding".into(), "identity".into()),
            ("user-agent".into(), "connect-go/1.18.1 (go1.26.3)".into()),
            ("connect-accept-encoding".into(), "gzip".into()),
        ]),
        body: encode_connect_frame(&request.encode_to_vec(), true, false)?,
    })
    .await?;
    validate_chat_response(response).await
}

async fn validate_chat_response(response: FetchResponse) -> Result<ByteStream, DevinError> {
    let status = response.status;
    let status_text = response.status_text.clone();
    if (200..300).contains(&status) {
        return response
            .body
            .ok_or_else(|| DevinError::protocol("Devin chat response body is empty"));
    }
    let body = read_all(response).await?;
    Err(DevinError::api(
        format!("Devin chat failed: {status} {status_text}"),
        status,
        Some(String::from_utf8_lossy(&body).into_owned()),
    ))
}

/// Fetches the user JWT and optional custom chat API base URL.
///
/// # Errors
///
/// Returns [`DevinError::Api`] for transport/HTTP failures and
/// [`DevinError::Protocol`] for malformed or empty responses.
pub async fn fetch_auth_metadata(
    token: &str,
    base_url: &str,
    fetch: FetchLike,
) -> Result<AuthMetadata, DevinError> {
    let request = GetUserJwtRequest {
        metadata: Some(metadata(token, "")),
    };
    let response = fetch(FetchRequest {
        url: format!("{base_url}{DEVIN_AUTH_PATH}"),
        method: "POST".into(),
        headers: IndexMap::from([
            ("content-type".into(), "application/proto".into()),
            ("connect-protocol-version".into(), "1".into()),
            ("accept".into(), "*/*".into()),
        ]),
        body: request.encode_to_vec(),
    })
    .await?;
    let status = response.status;
    let status_text = response.status_text.clone();
    let payload = read_all(response).await?;
    if !(200..300).contains(&status) {
        return Err(DevinError::api(
            format!("Devin auth failed: {status} {status_text}"),
            status,
            Some(String::from_utf8_lossy(&payload).into_owned()),
        ));
    }
    let decoded = decode_proto_with_gzip_fallback::<GetUserJwtResponse>(&payload)?;
    if decoded.user_jwt.is_empty() {
        return Err(DevinError::protocol(
            "Devin auth returned an empty user JWT",
        ));
    }
    let custom = decoded.custom_api_server_url.trim();
    Ok(AuthMetadata {
        user_jwt: decoded.user_jwt,
        base_url: (!custom.is_empty()).then(|| trim_trailing_slashes(custom)),
    })
}

pub fn build_chat_request(
    options: &StreamDevinChatOptions,
    token: &str,
    user_jwt: &str,
    uuid: &dyn Fn() -> String,
) -> GetChatMessageRequest {
    let cascade_id = options
        .conversation_id
        .as_ref()
        .or(options.session_id.as_ref())
        .cloned()
        .unwrap_or_else(uuid);
    let stop_patterns = DEVIN_DEFAULT_STOP_PATTERNS
        .iter()
        .map(ToString::to_string)
        .chain(options.stop_sequences.iter().cloned())
        .collect();
    let temperature = options.temperature.unwrap_or(0.4);
    GetChatMessageRequest {
        metadata: Some(metadata(token, user_jwt)),
        prompt: options.system_prompt.join("\n\n"),
        chat_message_prompts: build_chat_message_prompts(&options.messages, &cascade_id),
        chat_model_uid: options.model.clone(),
        request_type: ChatMessageRequestType::Cascade as i32,
        planner_mode: ConversationalPlannerMode::Default as i32,
        tool_choice: Some(ChatToolChoice {
            choice: Some(chat_tool_choice::Choice::OptionName("auto".into())),
        }),
        system_prompt_cache_options: Some(PromptCacheOptions {
            r#type: CacheControlType::Ephemeral as i32,
        }),
        disable_parallel_tool_calls: true,
        cascade_id,
        execution_id: uuid(),
        configuration: Some(CompletionConfiguration {
            num_completions: 1,
            max_tokens: options.max_tokens.unwrap_or(64_000),
            max_newlines: 200,
            temperature,
            first_temperature: temperature,
            top_k: 50,
            top_p: options.top_p.unwrap_or(1.0),
            stop_patterns,
            fim_eot_prob_threshold: 1.0,
        }),
        tools: options
            .tools
            .iter()
            .map(|tool| ChatToolDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                json_schema_string: tool.input_schema.to_string(),
                strict: tool.strict,
            })
            .collect(),
    }
}

pub fn build_chat_message_prompts(
    messages: &[DevinMessage],
    cascade_id: &str,
) -> Vec<ChatMessagePrompt> {
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| match message {
            DevinMessage::User { content } | DevinMessage::Developer { content } => {
                let normalized = normalize_content(content);
                ChatMessagePrompt {
                    message_id: crate::deterministic_uuid(&format!(
                        "{cascade_id}\0{index}\0{}",
                        if matches!(message, DevinMessage::User { .. }) {
                            "user"
                        } else {
                            "developer"
                        }
                    )),
                    source: ChatMessageSource::User as i32,
                    prompt: normalized.0,
                    images: normalized.1,
                    ..Default::default()
                }
            }
            DevinMessage::Assistant {
                content,
                response_id,
            } => {
                let normalized = normalize_assistant_content(content);
                ChatMessagePrompt {
                    message_id: response_id.clone().unwrap_or_else(|| {
                        format!(
                            "bot-{}",
                            crate::deterministic_uuid(&format!("{cascade_id}\0{index}\0assistant"))
                        )
                    }),
                    source: ChatMessageSource::System as i32,
                    prompt: normalized.text,
                    thinking: normalized.thinking,
                    signature: normalized.signature,
                    signature_type: String::new(),
                    tool_calls: normalized.tool_calls,
                    ..Default::default()
                }
            }
            DevinMessage::Tool {
                tool_call_id,
                content,
                is_error,
            } => {
                let normalized = normalize_content(content);
                ChatMessagePrompt {
                    message_id: crate::deterministic_uuid(&format!(
                        "{cascade_id}\0{index}\0tool\0{tool_call_id}"
                    )),
                    source: ChatMessageSource::Tool as i32,
                    tool_call_id: tool_call_id.clone(),
                    tool_result_is_error: *is_error,
                    prompt: normalized.0,
                    images: normalized.1,
                    ..Default::default()
                }
            }
        })
        .collect()
}

fn advance_streaming_tool_call(
    previous: Option<&StreamingToolCallState>,
    call: &ChatToolCall,
) -> (StreamingToolCallState, String, Option<Value>) {
    let previous_json = previous.map_or("", |state| state.arguments_json.as_str());
    let arguments_json = if call.arguments_json.starts_with(previous_json) {
        call.arguments_json.clone()
    } else {
        format!("{previous_json}{}", call.arguments_json)
    };
    let parsed = parse_possibly_complete_json_throttled(
        &arguments_json,
        previous.map_or(0, |state| state.last_parse_attempt_length),
    );
    let state = StreamingToolCallState {
        name: if call.name.is_empty() {
            previous.map_or_else(String::new, |state| state.name.clone())
        } else {
            call.name.clone()
        },
        arguments_json: arguments_json.clone(),
        last_parse_attempt_length: parsed.as_ref().map_or_else(
            || previous.map_or(0, |state| state.last_parse_attempt_length),
            |parsed| parsed.attempted_length,
        ),
    };
    let delta = arguments_json
        .get(previous_json.len()..)
        .unwrap_or_default()
        .to_owned();
    (state, delta, parsed.and_then(|parsed| parsed.value))
}

fn normalize_content(content: &[DevinContentPart]) -> (String, Vec<ImageData>) {
    content.iter().fold(
        (String::new(), Vec::new()),
        |(mut text, mut images), part| {
            match part {
                DevinContentPart::Text { text: part } => text.push_str(part),
                DevinContentPart::Image { data, mime_type } => images.push(ImageData {
                    base64_data: data.clone(),
                    mime_type: mime_type.clone(),
                }),
            }
            (text, images)
        },
    )
}

struct NormalizedAssistant {
    text: String,
    thinking: String,
    signature: String,
    tool_calls: Vec<ChatToolCall>,
}

fn normalize_assistant_content(content: &[DevinAssistantContentPart]) -> NormalizedAssistant {
    content.iter().fold(
        NormalizedAssistant {
            text: String::new(),
            thinking: String::new(),
            signature: String::new(),
            tool_calls: Vec::new(),
        },
        |mut normalized, part| {
            match part {
                DevinAssistantContentPart::Text { text } => {
                    normalized.text.push_str(text);
                }
                DevinAssistantContentPart::Thinking {
                    thinking,
                    thinking_signature,
                } => {
                    normalized.thinking.push_str(thinking);
                    if normalized.signature.is_empty() {
                        normalized.signature = thinking_signature.clone().unwrap_or_default();
                    }
                }
                DevinAssistantContentPart::ToolCall {
                    id,
                    name,
                    arguments,
                } => normalized.tool_calls.push(ChatToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments_json: arguments.to_string(),
                }),
            }
            normalized
        },
    )
}
