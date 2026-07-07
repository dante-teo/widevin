export const DEVIN_DEFAULT_BASE_URL = "https://server.codeium.com";
export const DEVIN_APP_BASE_URL = "https://app.devin.ai";
export const DEVIN_AUTH_BASE_URL = "https://api.devin.ai";
export const DEVIN_SESSION_TOKEN_PREFIX = "devin-session-token$";
export const DEVIN_IDE_VERSION = "3.2.23";
export const DEVIN_EXTENSION_VERSION = "1.48.2";
export const DEVIN_AUTH_PATH = "/exa.auth_pb.AuthService/GetUserJwt";
export const DEVIN_CHAT_PATH = "/exa.api_server_pb.ApiServerService/GetChatMessage";
export const DEVIN_MODELS_PATH = "/exa.api_server_pb.ApiServerService/GetCliModelConfigs";
export const DEVIN_OAUTH_CALLBACK_PORT = 59653;
export const DEVIN_OAUTH_CALLBACK_PATH = "/callback";
export const DEVIN_OAUTH_TOKEN_PATH = "/auth/cli/token";
export const DEVIN_DEFAULT_STOP_PATTERNS = [
  "<|user|>",
  "<|bot|>",
  "<|context_request|>",
  "<|endoftext|>",
  "<|end_of_turn|>"
] as const;
