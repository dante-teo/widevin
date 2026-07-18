//! Rust bindings for the Devin/Cascade OAuth, model, and streaming chat protocol.

mod auth;
mod chat;
mod connect;
mod constants;
mod error;
mod fetch;
mod json;
mod models;
#[doc(hidden)]
pub mod proto;
mod provider;
mod token;
mod types;
mod uuid;

pub use auth::{
    BuildDevinAuthUrlOptions, DevinLoginOptions, ExchangeDevinCliTokenOptions, PkcePair,
    build_devin_auth_url, create_pkce_pair, exchange_devin_cli_token, login_devin,
    validate_devin_oauth_callback,
};
pub use chat::{
    AuthMetadata, DevinEventStream, StreamDevinChatOptions, build_chat_message_prompts,
    build_chat_request, fetch_auth_metadata, stream_devin_chat,
};
pub use connect::{
    CONNECT_COMPRESSED_FLAG, CONNECT_END_STREAM_FLAG, ConnectFrame, MAX_CONNECT_FRAME_PAYLOAD,
    decode_connect_frames, encode_connect_frame, read_connect_trailer_error,
};
pub use error::DevinError;
pub use fetch::{
    ByteStream, FetchFuture, FetchLike, FetchRequest, FetchResponse, fetch_response_from_bytes,
    fetch_response_from_chunks,
};
pub use json::{
    STREAMING_JSON_PARSE_MIN_GROWTH, ThrottledJsonParse, parse_possibly_complete_json,
    parse_possibly_complete_json_throttled, parse_streaming_json,
};
pub use models::{ListDevinModelsOptions, list_devin_models, metadata, normalize_models};
pub use provider::{DevinProvider, create_devin_provider};
pub use token::{
    TokenGetFuture, TokenStore, TokenStoreFuture, TokenStoreRef, create_file_token_store,
    create_memory_token_store, create_token_store, normalize_devin_session_token,
};
pub use types::{
    DevinAssistantContentPart, DevinChatRequest, DevinContentPart, DevinInput, DevinMessage,
    DevinModel, DevinProviderOptions, DevinStopReason, DevinStreamEvent, DevinTool, OpenBrowser,
    OpenBrowserFuture, UuidGenerator,
};
pub use uuid::deterministic_uuid;
