import { gunzipSync } from "node:zlib";
import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import type { DescMessage, MessageInitShape, MessageShape } from "@bufbuild/protobuf";
import {
  ChatMessageRequestType,
  GetChatMessageRequestSchema,
  GetChatMessageResponseSchema,
  GetCliModelConfigsRequestSchema,
  GetCliModelConfigsResponseSchema
} from "./proto/generated/exa/api_server_pb/api_server_pb.js";
import { GetUserJwtRequestSchema, GetUserJwtResponseSchema } from "./proto/generated/exa/auth_pb/auth_pb.js";
import {
  CacheControlType,
  ChatMessagePromptSchema,
  ChatToolChoiceSchema,
  ChatToolDefinitionSchema,
  PromptCacheOptionsSchema
} from "./proto/generated/exa/chat_pb/chat_pb.js";
import {
  ChatMessageSource,
  ChatToolCallSchema,
  ClientModelConfigSchema,
  CompletionConfigurationSchema,
  ConversationalPlannerMode,
  ImageDataSchema,
  MetadataSchema,
  ModelFeaturesSchema,
  ModelInfoSchema,
  ModelUsageStatsSchema,
  StopReason
} from "./proto/generated/exa/codeium_common_pb/codeium_common_pb.js";

export {
  ChatMessageRequestType,
  ChatMessageSource,
  StopReason,
  CacheControlType,
  ConversationalPlannerMode
};

export const schemas = {
  getChatMessageRequest: GetChatMessageRequestSchema,
  getChatMessageResponse: GetChatMessageResponseSchema,
  getCliModelConfigsRequest: GetCliModelConfigsRequestSchema,
  getCliModelConfigsResponse: GetCliModelConfigsResponseSchema,
  getUserJwtRequest: GetUserJwtRequestSchema,
  getUserJwtResponse: GetUserJwtResponseSchema,
  metadata: MetadataSchema,
  chatMessagePrompt: ChatMessagePromptSchema,
  chatToolCall: ChatToolCallSchema,
  chatToolChoice: ChatToolChoiceSchema,
  chatToolDefinition: ChatToolDefinitionSchema,
  promptCacheOptions: PromptCacheOptionsSchema,
  completionConfiguration: CompletionConfigurationSchema,
  imageData: ImageDataSchema,
  clientModelConfig: ClientModelConfigSchema,
  modelInfo: ModelInfoSchema,
  modelFeatures: ModelFeaturesSchema,
  modelUsageStats: ModelUsageStatsSchema
} as const;

export const make = <Desc extends DescMessage>(schema: Desc, value?: MessageInitShape<Desc>): MessageShape<Desc> => create(schema, value);

export const encodeProto = <Desc extends DescMessage>(schema: Desc, value: MessageShape<Desc>): Uint8Array => toBinary(schema, value);

export const decodeProto = <Desc extends DescMessage>(schema: Desc, payload: Uint8Array): MessageShape<Desc> => fromBinary(schema, payload);

export const decodeProtoWithGzipFallback = <Desc extends DescMessage>(schema: Desc, payload: Uint8Array): MessageShape<Desc> => {
  try {
    return decodeProto(schema, payload);
  } catch (error) {
    try {
      return decodeProto(schema, gunzipSync(payload));
    } catch {
      throw error;
    }
  }
};
