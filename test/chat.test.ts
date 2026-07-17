import { create } from "@bufbuild/protobuf";
import { describe, expect, it, vi } from "vitest";
import { encodeConnectFrame } from "../src/devin/connect.js";
import { buildChatMessagePrompts, streamDevinChat } from "../src/devin/chat.js";
import { decodeProto, encodeProto, schemas } from "../src/devin/proto.js";
import { DevinApiError } from "../src/index.js";
import { collect, protoResponse, streamResponse } from "./helpers.js";

const collectChatEvents = async (chatFrames: readonly Uint8Array[]) => {
  const authPayload = encodeProto(schemas.getUserJwtResponse, create(schemas.getUserJwtResponse, { userJwt: "jwt-1" }));
  const fetch = vi
    .fn()
    .mockResolvedValueOnce(protoResponse(authPayload))
    .mockResolvedValueOnce(streamResponse([...chatFrames, encodeConnectFrame(new TextEncoder().encode("{}"), { endStream: true })]));

  return collect(streamDevinChat({ token: "raw", model: "model-a", messages: [], fetch }));
};

describe("chat streaming", () => {
  it("fetches JWT first, sends compressed chat request, and emits deltas/tool/usage/done", async () => {
    const authPayload = encodeProto(
      schemas.getUserJwtResponse,
      create(schemas.getUserJwtResponse, { userJwt: "jwt-1", customApiServerUrl: "https://chat.example/" })
    );
    const frames = [
      encodeConnectFrame(
        encodeProto(schemas.getChatMessageResponse, create(schemas.getChatMessageResponse, { deltaThinking: "think" })),
        { compress: true }
      ),
      encodeConnectFrame(encodeProto(schemas.getChatMessageResponse, create(schemas.getChatMessageResponse, { deltaText: "hi" })), {
        compress: true
      }),
      encodeConnectFrame(
        encodeProto(
          schemas.getChatMessageResponse,
          create(schemas.getChatMessageResponse, {
            deltaToolCalls: [create(schemas.chatToolCall, { id: "call-1", name: "search", argumentsJson: '{"q"' })]
          })
        ),
        { compress: true }
      ),
      encodeConnectFrame(
        encodeProto(
          schemas.getChatMessageResponse,
          create(schemas.getChatMessageResponse, {
            deltaToolCalls: [create(schemas.chatToolCall, { id: "call-1", argumentsJson: '{"q":"x"}' })],
            usage: create(schemas.modelUsageStats, { inputTokens: 2n, outputTokens: 3n, cacheReadTokens: 4n, cacheWriteTokens: 5n })
          })
        ),
        { compress: true }
      ),
      encodeConnectFrame(new TextEncoder().encode("{}"), { endStream: true })
    ];
    const fetch = vi
      .fn()
      .mockResolvedValueOnce(protoResponse(authPayload))
      .mockResolvedValueOnce(streamResponse(frames));

    const events = await collect(
      streamDevinChat({
        token: "raw",
        baseUrl: "https://server.example",
        model: "model-a",
        messages: [{ role: "user", content: "hello" }],
        fetch,
        uuid: () => "uuid-1"
      })
    );

    expect(fetch.mock.calls[0]?.[0]).toBe("https://server.example/exa.auth_pb.AuthService/GetUserJwt");
    expect(fetch.mock.calls[1]?.[0]).toBe("https://chat.example/exa.api_server_pb.ApiServerService/GetChatMessage");
    expect(new Uint8Array(fetch.mock.calls[1]?.[1]?.body as Uint8Array)[0]).toBe(1);
    expect(events).toEqual([
      { type: "thinking_delta", delta: "think" },
      { type: "text_delta", delta: "hi" },
      { type: "toolcall_start", id: "call-1", name: "search" },
      { type: "toolcall_delta", id: "call-1", delta: '{"q"' },
      { type: "toolcall_delta", id: "call-1", delta: ':"x"}' },
      { type: "usage", inputTokens: 2, outputTokens: 3, cacheReadTokens: 4, cacheWriteTokens: 5 },
      { type: "toolcall_end", id: "call-1", name: "search", arguments: { q: "x" } },
      { type: "done", reason: "toolUse" }
    ]);
  });

  it("accumulates genuinely incremental tool argument fragments", async () => {
    const events = await collectChatEvents([
      encodeConnectFrame(
        encodeProto(
          schemas.getChatMessageResponse,
          create(schemas.getChatMessageResponse, {
            deltaToolCalls: [
              create(schemas.chatToolCall, { id: "call-1", name: "search", argumentsJson: '{"q"' }),
              create(schemas.chatToolCall, { id: "call-1", argumentsJson: ':"x"}' })
            ]
          })
        ),
        { compress: true }
      )
    ]);

    expect(events.filter((event) => event.type === "toolcall_delta")).toEqual([
      { type: "toolcall_delta", id: "call-1", delta: '{"q"' },
      { type: "toolcall_delta", id: "call-1", delta: ':"x"}' }
    ]);
    expect(events.find((event) => event.type === "toolcall_end")).toEqual({
      type: "toolcall_end",
      id: "call-1",
      name: "search",
      arguments: { q: "x" }
    });
  });

  it("throttles cumulative tool argument parsing per tool-call ID and fully parses final arguments", async () => {
    const cumulativeArguments = Array.from({ length: 260 }, (_, index) => `{}${" ".repeat(index)}`);
    const toolCalls = [
      create(schemas.chatToolCall, { id: "call-a", name: "first", argumentsJson: cumulativeArguments[0]! }),
      create(schemas.chatToolCall, { id: "call-b", name: "second", argumentsJson: '{"id":"b"}' }),
      ...cumulativeArguments.slice(1).map((argumentsJson) => create(schemas.chatToolCall, { id: "call-a", argumentsJson }))
    ];
    const events = await collectChatEvents([
      encodeConnectFrame(
        encodeProto(schemas.getChatMessageResponse, create(schemas.getChatMessageResponse, { deltaToolCalls: toolCalls })),
        { compress: true }
      )
    ]);

    const firstDeltas = events.filter(
      (event): event is Extract<(typeof events)[number], { type: "toolcall_delta" }> =>
        event.type === "toolcall_delta" && event.id === "call-a"
    );
    const secondDelta = events.find((event) => event.type === "toolcall_delta" && event.id === "call-b");
    const firstEnd = events.find((event) => event.type === "toolcall_end" && event.id === "call-a");

    expect(firstDeltas.map((event) => event.delta).join("")).toBe(cumulativeArguments.at(-1));
    expect(
      firstDeltas.flatMap((event, index) => ("arguments" in event ? [index] : []))
    ).toEqual([0, 256]);
    expect(secondDelta).toEqual({ type: "toolcall_delta", id: "call-b", delta: '{"id":"b"}', arguments: { id: "b" } });
    expect(firstEnd).toEqual({ type: "toolcall_end", id: "call-a", name: "first", arguments: {} });
  });

  it("maps assistant tool calls and tool results into stable prompts", () => {
    const prompts = buildChatMessagePrompts(
      [
        {
          role: "assistant",
          responseId: "assistant-1",
          content: [{ type: "toolCall", id: "call-1", name: "search", arguments: { q: "x" } }]
        },
        { role: "tool", toolCallId: "call-1", isError: true, content: "result" }
      ],
      "cascade-1"
    );
    expect(prompts[0]?.messageId).toBe("assistant-1");
    expect(prompts[0]?.toolCalls[0]?.argumentsJson).toBe('{"q":"x"}');
    expect(prompts[1]?.toolCallId).toBe("call-1");
    expect(prompts[1]?.toolResultIsError).toBe(true);
  });

  it("throws for chat non-200 responses after auth", async () => {
    const authPayload = encodeProto(schemas.getUserJwtResponse, create(schemas.getUserJwtResponse, { userJwt: "jwt-1" }));
    const fetch = vi
      .fn()
      .mockResolvedValueOnce(protoResponse(authPayload))
      .mockResolvedValueOnce(new Response("nope", { status: 429, statusText: "Rate Limited" }));
    await expect(
      collect(streamDevinChat({ token: "raw", model: "model-a", messages: [], fetch }))
    ).rejects.toBeInstanceOf(DevinApiError);
  });
});
