import { gzipSync } from "node:zlib";
import { create } from "@bufbuild/protobuf";
import { describe, expect, it, vi } from "vitest";
import { DevinApiError, listDevinModels } from "../src/index.js";
import { decodeProto, encodeProto, schemas } from "../src/devin/proto.js";
import { protoResponse } from "./helpers.js";

describe("model discovery", () => {
  it("normalizes enabled models and sends prefixed token metadata", async () => {
    const payload = encodeProto(
      schemas.getCliModelConfigsResponse,
      create(schemas.getCliModelConfigsResponse, {
        clientModelConfigs: [
          create(schemas.clientModelConfig, {
            modelUid: " model-a ",
            label: "Model A Thinking",
            supportsImages: true,
            maxTokens: 1000
          }),
          create(schemas.clientModelConfig, { modelUid: "disabled", label: "Disabled", disabled: true }),
          create(schemas.clientModelConfig, { modelUid: " ", label: "Blank" })
        ]
      })
    );
    const fetch = vi.fn(async () => protoResponse(gzipSync(payload)));
    const models = await listDevinModels({ token: "raw", baseUrl: "https://server.example", fetch });
    expect(models).toEqual([
      {
        id: "model-a",
        name: "Model A Thinking",
        provider: "devin",
        baseUrl: "https://server.example",
        input: ["text", "image"],
        supportsTools: true,
        reasoning: true,
        contextWindow: 1000,
        maxTokens: 1000
      }
    ]);
    const call = fetch.mock.calls[0] as unknown as [string, RequestInit];
    const request = decodeProto(schemas.getCliModelConfigsRequest, call[1].body as Uint8Array);
    expect(request.metadata?.apiKey).toBe("devin-session-token$raw");
  });

  it("throws DevinApiError for non-200 responses", async () => {
    const fetch = vi.fn(async () => new Response("nope", { status: 500, statusText: "Nope" }));
    await expect(listDevinModels({ fetch })).rejects.toBeInstanceOf(DevinApiError);
  });
});
