import {
  DEVIN_DEFAULT_BASE_URL,
  DEVIN_EXTENSION_VERSION,
  DEVIN_IDE_VERSION,
  DEVIN_MODELS_PATH
} from "./constants.js";
import { DevinApiError } from "./errors.js";
import { decodeProtoWithGzipFallback, encodeProto, make, schemas } from "./proto.js";
import { normalizeDevinSessionToken } from "./token.js";
import type { DevinModel, FetchLike } from "./types.js";

const DEFAULT_CONTEXT_WINDOW = 200_000;
const DEFAULT_MAX_TOKENS = 64_000;
const REASONING_LABEL_PATTERN = /think|thinking|minimal|high|medium|low|xhigh|max|reasoning/iu;
const NO_REASONING_LABEL_PATTERN = /\bno thinking\b/iu;

export interface ListDevinModelsOptions {
  token?: string;
  baseUrl?: string;
  fetch?: FetchLike;
  signal?: AbortSignal;
}

export const listDevinModels = async ({
  token,
  baseUrl = DEVIN_DEFAULT_BASE_URL,
  fetch: fetchImpl = fetch,
  signal
}: ListDevinModelsOptions): Promise<readonly DevinModel[]> => {
  const request = make(schemas.getCliModelConfigsRequest, {
    metadata: make(schemas.metadata, metadata(normalizeDevinSessionToken(token)))
  });
  const response = await fetchImpl(`${baseUrl.replace(/\/+$/, "")}${DEVIN_MODELS_PATH}`, {
    method: "POST",
    headers: {
      "content-type": "application/proto",
      "connect-protocol-version": "1",
      accept: "*/*"
    },
    body: encodeProto(schemas.getCliModelConfigsRequest, request) as BodyInit,
    ...(signal ? { signal } : {})
  });
  const payload = new Uint8Array(await response.arrayBuffer());
  if (!response.ok) {
    throw new DevinApiError(`Devin model discovery failed: ${response.status} ${response.statusText}`, response.status, text(payload));
  }
  const decoded = decodeProtoWithGzipFallback(schemas.getCliModelConfigsResponse, payload);
  return normalizeModels(decoded.clientModelConfigs, baseUrl);
};

export const normalizeModels = (configs: readonly { disabled: boolean; modelUid: string; label: string; supportsImages: boolean; maxTokens: number; modelInfo?: { modelFeatures?: { supportsThinking: boolean } | undefined } | undefined }[], baseUrl: string): readonly DevinModel[] =>
  [...configs
    .filter((config) => !config.disabled)
    .map((config) => ({ ...config, modelUid: config.modelUid.trim() }))
    .filter((config) => config.modelUid.length > 0)
    .reduce((byId, config) => {
      const contextWindow = config.maxTokens > 0 ? config.maxTokens : DEFAULT_CONTEXT_WINDOW;
      return byId.set(config.modelUid, {
        id: config.modelUid,
        name: config.label.trim() || config.modelUid,
        provider: "devin" as const,
        baseUrl,
        input: config.supportsImages ? (["text", "image"] as const) : (["text"] as const),
        supportsTools: true as const,
        reasoning: supportsThinking(config),
        contextWindow,
        maxTokens: Math.min(config.maxTokens > 0 ? config.maxTokens : DEFAULT_MAX_TOKENS, DEFAULT_MAX_TOKENS)
      });
    }, new Map<string, DevinModel>())
    .values()].sort((a, b) => a.id.localeCompare(b.id));

export const metadata = (apiKey: string, userJwt = "") => ({
  apiKey,
  userJwt,
  ideName: "windsurf",
  ideVersion: DEVIN_IDE_VERSION,
  extensionName: "windsurf",
  extensionVersion: DEVIN_EXTENSION_VERSION,
  locale: "en"
});

const supportsThinking = (config: { label: string; modelInfo?: { modelFeatures?: { supportsThinking: boolean } | undefined } | undefined }): boolean =>
  NO_REASONING_LABEL_PATTERN.test(config.label)
    ? false
    : config.modelInfo?.modelFeatures?.supportsThinking === true || REASONING_LABEL_PATTERN.test(config.label);

const text = (bytes: Uint8Array): string => new TextDecoder().decode(bytes);
