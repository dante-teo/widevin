export type FetchLike = typeof fetch;

export interface TokenStore {
  get(): Promise<string | undefined>;
  set(token: string): Promise<void>;
  clear(): Promise<void>;
}

export interface DevinProviderOptions {
  tokenStore?: TokenStore;
  fetch?: FetchLike;
  baseUrl?: string;
  authBaseUrl?: string;
  appBaseUrl?: string;
  openBrowser?: (url: string) => Promise<void> | void;
  uuid?: () => string;
}

export interface DevinModel {
  id: string;
  name: string;
  provider: "devin";
  baseUrl: string;
  input: readonly ("text" | "image")[];
  supportsTools: true;
  reasoning: boolean;
  contextWindow: number;
  maxTokens: number;
}

export type DevinMessage =
  | { role: "user" | "developer"; content: string | readonly DevinContentPart[] }
  | { role: "assistant"; content: readonly DevinAssistantContentPart[]; responseId?: string }
  | { role: "tool"; toolCallId: string; content: string | readonly DevinContentPart[]; isError?: boolean };

export type DevinContentPart =
  | { type: "text"; text: string }
  | { type: "image"; data: string; mimeType: string };

export type DevinAssistantContentPart =
  | { type: "text"; text: string }
  | { type: "thinking"; thinking: string; thinkingSignature?: string }
  | { type: "toolCall"; id: string; name: string; arguments: unknown };

export interface DevinTool {
  name: string;
  description: string;
  inputSchema: unknown;
  strict?: boolean;
}

export interface DevinChatRequest {
  model: string;
  messages: readonly DevinMessage[];
  /**
   * Joined with `"\n\n"` into the request's `prompt` field. Optional in
   * general, but **mandatory when `model` is a Claude series model and
   * `tools` is non-empty** — Claude's tool-use path requires a non-empty
   * system prompt; omitting it yields degraded or rejected tool calls.
   */
  systemPrompt?: readonly string[];
  tools?: readonly DevinTool[];
  conversationId?: string;
  sessionId?: string;
  maxTokens?: number;
  temperature?: number;
  topP?: number;
  stopSequences?: readonly string[];
  signal?: AbortSignal;
}

export type DevinStreamEvent =
  | { type: "text_delta"; delta: string }
  | { type: "thinking_delta"; delta: string; signature?: string }
  | { type: "toolcall_start"; id: string; name: string }
  | {
      type: "toolcall_delta";
      id: string;
      /** Raw newly received suffix; emitted for every tool-argument update. */
      delta: string;
      /** Opportunistic parsed snapshot. It can lag `delta` because parsing is throttled. */
      arguments?: unknown;
    }
  | {
      type: "toolcall_end";
      id: string;
      name: string;
      /** Authoritative parse of the complete accumulated argument buffer. */
      arguments: unknown;
    }
  | { type: "usage"; inputTokens: number; outputTokens: number; cacheReadTokens: number; cacheWriteTokens: number }
  | { type: "done"; reason: "stop" | "length" | "toolUse" };

export interface DevinProvider {
  login(): Promise<string>;
  setToken(token: string): Promise<void>;
  clearToken(): Promise<void>;
  listModels(): Promise<readonly DevinModel[]>;
  streamChat(request: DevinChatRequest): AsyncIterable<DevinStreamEvent>;
}
