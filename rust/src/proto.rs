//! Minimal private wire schema. Only fields used by this crate are represented.

#[derive(Clone, PartialEq, prost::Message)]
pub struct Metadata {
    #[prost(string, tag = "1")]
    pub ide_name: String,
    #[prost(string, tag = "7")]
    pub ide_version: String,
    #[prost(string, tag = "12")]
    pub extension_name: String,
    #[prost(string, tag = "2")]
    pub extension_version: String,
    #[prost(string, tag = "3")]
    pub api_key: String,
    #[prost(string, tag = "4")]
    pub locale: String,
    #[prost(string, tag = "21")]
    pub user_jwt: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct GetUserJwtRequest {
    #[prost(message, optional, tag = "1")]
    pub metadata: Option<Metadata>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct GetUserJwtResponse {
    #[prost(string, tag = "1")]
    pub user_jwt: String,
    #[prost(string, tag = "2")]
    pub custom_api_server_url: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct GetCliModelConfigsRequest {
    #[prost(message, optional, tag = "1")]
    pub metadata: Option<Metadata>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct GetCliModelConfigsResponse {
    #[prost(message, repeated, tag = "1")]
    pub client_model_configs: Vec<ClientModelConfig>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ClientModelConfig {
    #[prost(string, tag = "1")]
    pub label: String,
    #[prost(string, tag = "22")]
    pub model_uid: String,
    #[prost(bool, tag = "4")]
    pub disabled: bool,
    #[prost(bool, tag = "5")]
    pub supports_images: bool,
    #[prost(int32, tag = "18")]
    pub max_tokens: i32,
    #[prost(message, optional, tag = "23")]
    pub model_info: Option<ModelInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ModelInfo {
    #[prost(message, optional, tag = "6")]
    pub model_features: Option<ModelFeatures>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ModelFeatures {
    #[prost(bool, tag = "15")]
    pub supports_thinking: bool,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ImageData {
    #[prost(string, tag = "1")]
    pub base64_data: String,
    #[prost(string, tag = "2")]
    pub mime_type: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ChatToolCall {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(string, tag = "3")]
    pub arguments_json: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ChatMessagePrompt {
    #[prost(string, tag = "1")]
    pub message_id: String,
    #[prost(enumeration = "ChatMessageSource", tag = "2")]
    pub source: i32,
    #[prost(string, tag = "3")]
    pub prompt: String,
    #[prost(message, repeated, tag = "6")]
    pub tool_calls: Vec<ChatToolCall>,
    #[prost(string, tag = "7")]
    pub tool_call_id: String,
    #[prost(bool, tag = "9")]
    pub tool_result_is_error: bool,
    #[prost(message, repeated, tag = "10")]
    pub images: Vec<ImageData>,
    #[prost(string, tag = "11")]
    pub thinking: String,
    #[prost(string, tag = "12")]
    pub signature: String,
    #[prost(string, tag = "18")]
    pub signature_type: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PromptCacheOptions {
    #[prost(enumeration = "CacheControlType", tag = "1")]
    pub r#type: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ChatToolDefinition {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub description: String,
    #[prost(string, tag = "3")]
    pub json_schema_string: String,
    #[prost(bool, tag = "12")]
    pub strict: bool,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ChatToolChoice {
    #[prost(oneof = "chat_tool_choice::Choice", tags = "1, 2")]
    pub choice: Option<chat_tool_choice::Choice>,
}

pub mod chat_tool_choice {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Choice {
        #[prost(string, tag = "1")]
        OptionName(String),
        #[prost(string, tag = "2")]
        ToolName(String),
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct CompletionConfiguration {
    #[prost(uint64, tag = "1")]
    pub num_completions: u64,
    #[prost(uint64, tag = "2")]
    pub max_tokens: u64,
    #[prost(uint64, tag = "3")]
    pub max_newlines: u64,
    #[prost(double, tag = "5")]
    pub temperature: f64,
    #[prost(double, tag = "6")]
    pub first_temperature: f64,
    #[prost(uint64, tag = "7")]
    pub top_k: u64,
    #[prost(double, tag = "8")]
    pub top_p: f64,
    #[prost(string, repeated, tag = "9")]
    pub stop_patterns: Vec<String>,
    #[prost(double, tag = "11")]
    pub fim_eot_prob_threshold: f64,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct GetChatMessageRequest {
    #[prost(message, optional, tag = "1")]
    pub metadata: Option<Metadata>,
    #[prost(string, tag = "2")]
    pub prompt: String,
    #[prost(message, repeated, tag = "3")]
    pub chat_message_prompts: Vec<ChatMessagePrompt>,
    #[prost(string, tag = "21")]
    pub chat_model_uid: String,
    #[prost(enumeration = "ChatMessageRequestType", tag = "7")]
    pub request_type: i32,
    #[prost(message, optional, tag = "8")]
    pub configuration: Option<CompletionConfiguration>,
    #[prost(message, repeated, tag = "10")]
    pub tools: Vec<ChatToolDefinition>,
    #[prost(bool, tag = "11")]
    pub disable_parallel_tool_calls: bool,
    #[prost(message, optional, tag = "12")]
    pub tool_choice: Option<ChatToolChoice>,
    #[prost(message, optional, tag = "13")]
    pub system_prompt_cache_options: Option<PromptCacheOptions>,
    #[prost(string, tag = "16")]
    pub cascade_id: String,
    #[prost(enumeration = "ConversationalPlannerMode", tag = "20")]
    pub planner_mode: i32,
    #[prost(string, tag = "22")]
    pub execution_id: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct GetChatMessageResponse {
    #[prost(string, tag = "3")]
    pub delta_text: String,
    #[prost(enumeration = "StopReason", tag = "5")]
    pub stop_reason: i32,
    #[prost(message, repeated, tag = "6")]
    pub delta_tool_calls: Vec<ChatToolCall>,
    #[prost(message, optional, tag = "7")]
    pub usage: Option<ModelUsageStats>,
    #[prost(string, tag = "9")]
    pub delta_thinking: String,
    #[prost(string, tag = "10")]
    pub delta_signature: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ModelUsageStats {
    #[prost(uint64, tag = "2")]
    pub input_tokens: u64,
    #[prost(uint64, tag = "3")]
    pub output_tokens: u64,
    #[prost(uint64, tag = "4")]
    pub cache_write_tokens: u64,
    #[prost(uint64, tag = "5")]
    pub cache_read_tokens: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum ChatMessageRequestType {
    Unspecified = 0,
    Cascade = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum ChatMessageSource {
    Unspecified = 0,
    User = 1,
    System = 2,
    Tool = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum ConversationalPlannerMode {
    Unspecified = 0,
    Default = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum CacheControlType {
    Unspecified = 0,
    Ephemeral = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum StopReason {
    Unspecified = 0,
    Incomplete = 1,
    StopPattern = 2,
    MaxTokens = 3,
    MinLogProb = 4,
    MaxNewlines = 5,
    ExitScope = 6,
    NonfiniteLogitOrProb = 7,
    FirstNonWhitespaceLine = 8,
    Partial = 9,
    FunctionCall = 10,
    ContentFilter = 11,
    NonInsertion = 12,
    Error = 13,
}
