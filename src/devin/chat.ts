import { DEVIN_AUTH_PATH, DEVIN_CHAT_PATH, DEVIN_DEFAULT_BASE_URL, DEVIN_DEFAULT_STOP_PATTERNS } from "./constants.js";
import type { MessageShape } from "@bufbuild/protobuf";
import { decodeConnectFrames, encodeConnectFrame, readConnectTrailerError, responseBodyToAsyncIterable } from "./connect.js";
import { DevinApiError, DevinProtocolError } from "./errors.js";
import { parsePossiblyCompleteJsonThrottled, parseStreamingJson } from "./json.js";
import { metadata } from "./models.js";
import {
  CacheControlType,
  ChatMessageRequestType,
  ChatMessageSource,
  ConversationalPlannerMode,
  decodeProto,
  decodeProtoWithGzipFallback,
  encodeProto,
  make,
  schemas,
  StopReason
} from "./proto.js";
import { normalizeDevinSessionToken } from "./token.js";
import type { DevinAssistantContentPart, DevinChatRequest, DevinContentPart, DevinMessage, DevinStreamEvent, FetchLike } from "./types.js";
import { deterministicUuid } from "./uuid.js";

type ImageDataMessage = MessageShape<typeof schemas.imageData>;
type ChatToolCallMessage = MessageShape<typeof schemas.chatToolCall>;
type StreamingToolCallState = Readonly<{
  name: string;
  argumentsJson: string;
  lastParseAttemptLength: number;
}>;

const advanceStreamingToolCall = (previous: StreamingToolCallState | undefined, call: ChatToolCallMessage) => {
  const previousJson = previous?.argumentsJson ?? "";
  const argumentsJson = call.argumentsJson.startsWith(previousJson) ? call.argumentsJson : previousJson + call.argumentsJson;
  const parsed = parsePossiblyCompleteJsonThrottled(argumentsJson, previous?.lastParseAttemptLength ?? 0);
  return {
    state: {
      name: call.name || previous?.name || "",
      argumentsJson,
      lastParseAttemptLength: parsed?.attemptedLength ?? previous?.lastParseAttemptLength ?? 0
    },
    delta: argumentsJson.slice(previousJson.length),
    parsedArguments: parsed?.value
  } as const;
};

export interface StreamDevinChatOptions extends DevinChatRequest {
  token?: string;
  baseUrl?: string;
  fetch?: FetchLike;
  uuid?: () => string;
}

export async function* streamDevinChat(options: StreamDevinChatOptions): AsyncIterable<DevinStreamEvent> {
  const fetchImpl = options.fetch ?? fetch;
  const baseUrl = (options.baseUrl ?? DEVIN_DEFAULT_BASE_URL).replace(/\/+$/, "");
  const token = normalizeDevinSessionToken(options.token);
  const auth = await fetchAuthMetadata({ token, baseUrl, fetch: fetchImpl, ...(options.signal ? { signal: options.signal } : {}) });
  const chatBaseUrl = auth.baseUrl ?? baseUrl;
  const request = buildChatRequest({ ...options, token, userJwt: auth.userJwt, uuid: options.uuid ?? (() => crypto.randomUUID()) });
  const response = await fetchImpl(`${chatBaseUrl}${DEVIN_CHAT_PATH}`, {
    method: "POST",
    headers: {
      "content-type": "application/connect+proto",
      "connect-protocol-version": "1",
      "connect-content-encoding": "gzip",
      "accept-encoding": "identity",
      "user-agent": "connect-go/1.18.1 (go1.26.3)",
      "connect-accept-encoding": "gzip"
    },
    body: encodeConnectFrame(encodeProto(schemas.getChatMessageRequest, request), { compress: true }) as BodyInit,
    ...(options.signal ? { signal: options.signal } : {})
  });
  if (!response.ok) {
    throw new DevinApiError(`Devin chat failed: ${response.status} ${response.statusText}`, response.status, await readResponseText(response));
  }
  if (!response.body) throw new DevinProtocolError("Devin chat response body is empty");

  const toolCalls = new Map<string, StreamingToolCallState>();
  let activeToolCallId: string | undefined;
  let sawToolCall = false;
  let latestStopReason = StopReason.UNSPECIFIED;

  for await (const frame of decodeConnectFrames(responseBodyToAsyncIterable(response.body))) {
    if (frame.endStream) {
      const trailerError = readConnectTrailerError(frame.payload);
      if (trailerError) throw new DevinProtocolError(trailerError);
      continue;
    }
    const message = decodeProto(schemas.getChatMessageResponse, frame.payload);
    if (message.deltaThinking) {
      yield { type: "thinking_delta", delta: message.deltaThinking, ...(message.deltaSignature ? { signature: message.deltaSignature } : {}) };
    }
    if (message.deltaText) {
      yield { type: "text_delta", delta: message.deltaText };
    }
    for (const call of message.deltaToolCalls) {
      const id = call.id || activeToolCallId;
      if (!id) continue;
      const previous = toolCalls.get(id);
      const { state, delta, parsedArguments } = advanceStreamingToolCall(previous, call);
      toolCalls.set(id, state);
      activeToolCallId = id;
      sawToolCall = true;
      if (!previous) yield { type: "toolcall_start", id, name: state.name };
      yield {
        type: "toolcall_delta",
        id,
        delta,
        ...(parsedArguments !== undefined ? { arguments: parsedArguments } : {})
      };
    }
    if (message.stopReason !== StopReason.UNSPECIFIED) latestStopReason = message.stopReason;
    if (message.usage) {
      yield {
        type: "usage",
        inputTokens: Number(message.usage.inputTokens),
        outputTokens: Number(message.usage.outputTokens),
        cacheReadTokens: Number(message.usage.cacheReadTokens),
        cacheWriteTokens: Number(message.usage.cacheWriteTokens)
      };
    }
  }
  for (const [id, { name, argumentsJson }] of toolCalls) {
    yield { type: "toolcall_end", id, name, arguments: parseStreamingJson(argumentsJson) };
  }
  yield { type: "done", reason: sawToolCall ? "toolUse" : latestStopReason === StopReason.MAX_TOKENS ? "length" : "stop" };
}

export const fetchAuthMetadata = async ({ token, baseUrl, fetch: fetchImpl, signal }: { token: string; baseUrl: string; fetch: FetchLike; signal?: AbortSignal }) => {
  const request = make(schemas.getUserJwtRequest, {
    metadata: make(schemas.metadata, metadata(token))
  });
  const response = await fetchImpl(`${baseUrl}${DEVIN_AUTH_PATH}`, {
    method: "POST",
    headers: {
      "content-type": "application/proto",
      "connect-protocol-version": "1",
      accept: "*/*"
    },
    body: encodeProto(schemas.getUserJwtRequest, request) as BodyInit,
    ...(signal ? { signal } : {})
  });
  const payload = new Uint8Array(await response.arrayBuffer());
  if (!response.ok) throw new DevinApiError(`Devin auth failed: ${response.status} ${response.statusText}`, response.status, new TextDecoder().decode(payload));
  const decoded = decodeProtoWithGzipFallback(schemas.getUserJwtResponse, payload);
  if (!decoded.userJwt) throw new DevinProtocolError("Devin auth returned an empty user JWT");
  const custom = decoded.customApiServerUrl.trim();
  return { userJwt: decoded.userJwt, ...(custom ? { baseUrl: custom.replace(/\/+$/, "") } : {}) };
};

const readResponseText = async (response: Response): Promise<string> => new TextDecoder().decode(await response.arrayBuffer());

export const buildChatRequest = (options: StreamDevinChatOptions & { token: string; userJwt: string; uuid: () => string }) => {
  const cascadeId = options.conversationId ?? options.sessionId ?? options.uuid();
  const stopPatterns = [...DEVIN_DEFAULT_STOP_PATTERNS, ...(options.stopSequences ?? [])];
  return make(schemas.getChatMessageRequest, {
    metadata: make(schemas.metadata, metadata(options.token, options.userJwt)),
    prompt: (options.systemPrompt ?? []).join("\n\n"),
    chatMessagePrompts: buildChatMessagePrompts(options.messages, cascadeId),
    chatModelUid: options.model,
    requestType: ChatMessageRequestType.CASCADE,
    plannerMode: ConversationalPlannerMode.DEFAULT,
    toolChoice: make(schemas.chatToolChoice, { choice: { case: "optionName", value: "auto" } }),
    systemPromptCacheOptions: make(schemas.promptCacheOptions, { type: CacheControlType.EPHEMERAL }),
    disableParallelToolCalls: true,
    cascadeId,
    executionId: options.uuid(),
    configuration: make(schemas.completionConfiguration, {
      numCompletions: 1n,
      maxTokens: BigInt(options.maxTokens ?? 64_000),
      maxNewlines: 200n,
      temperature: options.temperature ?? 0.4,
      firstTemperature: options.temperature ?? 0.4,
      topK: 50n,
      topP: options.topP ?? 1,
      stopPatterns,
      fimEotProbThreshold: 1
    }),
    tools: (options.tools ?? []).map((tool) =>
      make(schemas.chatToolDefinition, {
        name: tool.name,
        description: tool.description,
        jsonSchemaString: JSON.stringify(tool.inputSchema),
        strict: tool.strict ?? false
      })
    )
  });
};

export const buildChatMessagePrompts = (messages: readonly DevinMessage[], cascadeId: string) =>
  messages.map((message, index) => {
    switch (message.role) {
      case "user":
      case "developer": {
        const normalized = normalizeContent(message.content);
        return make(schemas.chatMessagePrompt, {
          messageId: deterministicUuid(`${cascadeId}\0${index}\0${message.role}`),
          source: ChatMessageSource.USER,
          prompt: normalized.text,
          images: normalized.images
        });
      }
      case "assistant": {
        const normalized = normalizeAssistantContent(message.content);
        return make(schemas.chatMessagePrompt, {
          messageId: message.responseId ?? `bot-${deterministicUuid(`${cascadeId}\0${index}\0assistant`)}`,
          source: ChatMessageSource.SYSTEM,
          prompt: normalized.text,
          thinking: normalized.thinking,
          signature: normalized.signature,
          signatureType: "",
          toolCalls: normalized.toolCalls
        });
      }
      case "tool": {
        const normalized = normalizeContent(message.content);
        return make(schemas.chatMessagePrompt, {
          messageId: deterministicUuid(`${cascadeId}\0${index}\0tool\0${message.toolCallId}`),
          source: ChatMessageSource.TOOL,
          toolCallId: message.toolCallId,
          toolResultIsError: message.isError ?? false,
          prompt: normalized.text,
          images: normalized.images
        });
      }
    }
  });

const normalizeContent = (content: string | readonly DevinContentPart[]) =>
  typeof content === "string"
    ? { text: content, images: [] }
    : content.reduce<{ text: string; images: ImageDataMessage[] }>(
        (acc, part) =>
          part.type === "text"
            ? { ...acc, text: acc.text + part.text }
            : { ...acc, images: [...acc.images, make(schemas.imageData, { base64Data: part.data, mimeType: part.mimeType })] },
        { text: "", images: [] }
      );

const normalizeAssistantContent = (content: readonly DevinAssistantContentPart[]) =>
  content.reduce<{ text: string; thinking: string; signature: string; toolCalls: ChatToolCallMessage[] }>(
    (acc, part) => {
      if (part.type === "text") return { ...acc, text: acc.text + part.text };
      if (part.type === "thinking") return { ...acc, thinking: acc.thinking + part.thinking, signature: acc.signature || part.thinkingSignature || "" };
      return {
        ...acc,
        toolCalls: [
          ...acc.toolCalls,
          make(schemas.chatToolCall, {
            id: part.id,
            name: part.name,
            argumentsJson: JSON.stringify(part.arguments)
          })
        ]
      };
    },
    { text: "", thinking: "", signature: "", toolCalls: [] }
  );
