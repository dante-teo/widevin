import { gunzipSync, gzipSync } from "node:zlib";
import { DevinProtocolError } from "./errors.js";

export const CONNECT_COMPRESSED_FLAG = 0x01;
export const CONNECT_END_STREAM_FLAG = 0x02;
export const MAX_CONNECT_FRAME_PAYLOAD = 16 * 1024 * 1024;

export interface ConnectFrame {
  flags: number;
  payload: Uint8Array<ArrayBufferLike>;
  endStream: boolean;
}

export const encodeConnectFrame = (payload: Uint8Array, options: { compress?: boolean; endStream?: boolean } = {}): Uint8Array => {
  const body = options.compress ? gzipSync(payload) : payload;
  const frame = new Uint8Array(5 + body.length);
  const view = new DataView(frame.buffer, frame.byteOffset, frame.byteLength);
  view.setUint8(0, (options.compress ? CONNECT_COMPRESSED_FLAG : 0) | (options.endStream ? CONNECT_END_STREAM_FLAG : 0));
  view.setUint32(1, body.length, false);
  frame.set(body, 5);
  return frame;
};

export const decodeConnectPayload = (flags: number, payload: Uint8Array): Uint8Array<ArrayBufferLike> => {
  const decoded = flags & CONNECT_COMPRESSED_FLAG ? gunzipSync(payload) : payload;
  return new Uint8Array(decoded);
};

export const readConnectTrailerError = (payload: Uint8Array): string | undefined => {
  const text = new TextDecoder().decode(payload).trim();
  if (!text) return undefined;
  try {
    const parsed = JSON.parse(text) as { error?: { code?: unknown; message?: unknown } };
    const code = typeof parsed.error?.code === "string" ? parsed.error.code : "";
    const message = typeof parsed.error?.message === "string" ? parsed.error.message : "";
    return code || message ? `Devin stream error${code ? ` ${code}` : ""}: ${message}` : undefined;
  } catch {
    return undefined;
  }
};

export async function* decodeConnectFrames(chunks: Iterable<Uint8Array> | AsyncIterable<Uint8Array>): AsyncIterable<ConnectFrame> {
  let pending: Uint8Array<ArrayBufferLike> = new Uint8Array();
  for await (const chunk of chunks) {
    pending = concatBytes([pending, chunk]);
    for (;;) {
      if (pending.length < 5) break;
      const view = new DataView(pending.buffer, pending.byteOffset, pending.byteLength);
      const flags = view.getUint8(0);
      const length = view.getUint32(1, false);
      if (length > MAX_CONNECT_FRAME_PAYLOAD) {
        throw new DevinProtocolError(`Devin Connect frame length ${length} exceeds ${MAX_CONNECT_FRAME_PAYLOAD}-byte cap`);
      }
      if (pending.length < 5 + length) break;
      const payload = pending.slice(5, 5 + length);
      pending = pending.slice(5 + length);
      yield {
        flags,
        payload: decodeConnectPayload(flags, payload),
        endStream: Boolean(flags & CONNECT_END_STREAM_FLAG)
      };
    }
  }
  if (pending.length > 0) {
    throw new DevinProtocolError("Devin Connect stream ended with a partial frame");
  }
}

export const responseBodyToAsyncIterable = (body: ReadableStream<Uint8Array>): AsyncIterable<Uint8Array> => ({
  async *[Symbol.asyncIterator]() {
    const reader = body.getReader();
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (done) return;
        if (value) yield value;
      }
    } finally {
      reader.releaseLock();
    }
  }
});

const concatBytes = (chunks: readonly Uint8Array[]): Uint8Array => {
  const length = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const output = new Uint8Array(length);
  chunks.reduce((offset, chunk) => {
    output.set(chunk, offset);
    return offset + chunk.length;
  }, 0);
  return output;
};
