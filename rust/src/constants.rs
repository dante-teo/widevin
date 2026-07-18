pub const DEVIN_DEFAULT_BASE_URL: &str = "https://server.codeium.com";
pub const DEVIN_APP_BASE_URL: &str = "https://app.devin.ai";
pub const DEVIN_AUTH_BASE_URL: &str = "https://api.devin.ai";
pub const DEVIN_SESSION_TOKEN_PREFIX: &str = "devin-session-token$";
pub const DEVIN_IDE_VERSION: &str = "3.2.23";
pub const DEVIN_EXTENSION_VERSION: &str = "1.48.2";
pub const DEVIN_AUTH_PATH: &str = "/exa.auth_pb.AuthService/GetUserJwt";
pub const DEVIN_CHAT_PATH: &str = "/exa.api_server_pb.ApiServerService/GetChatMessage";
pub const DEVIN_MODELS_PATH: &str = "/exa.api_server_pb.ApiServerService/GetCliModelConfigs";
pub const DEVIN_OAUTH_CALLBACK_PORT: u16 = 59_653;
pub const DEVIN_OAUTH_CALLBACK_PATH: &str = "/callback";
pub const DEVIN_OAUTH_TOKEN_PATH: &str = "/auth/cli/token";
pub const DEVIN_DEFAULT_STOP_PATTERNS: [&str; 5] = [
    "<|user|>",
    "<|bot|>",
    "<|context_request|>",
    "<|endoftext|>",
    "<|end_of_turn|>",
];

pub fn trim_trailing_slashes(value: &str) -> String {
    value.trim_end_matches('/').to_owned()
}
