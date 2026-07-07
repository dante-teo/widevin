import { gzipSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import { decodeConnectFrames, encodeConnectFrame, MAX_CONNECT_FRAME_PAYLOAD } from "../src/devin/connect.js";
import { DevinProtocolError } from "../src/index.js";
import { collect } from "./helpers.js";

describe("Connect framing", () => {
  it("encodes compressed frames and decodes split chunks", async () => {
    const frame = encodeConnectFrame(new TextEncoder().encode("hello"), { compress: true });
    expect(frame[0]).toBe(1);
    const decoded = await collect(decodeConnectFrames([frame.slice(0, 3), frame.slice(3)]));
    expect(new TextDecoder().decode(decoded[0]?.payload)).toBe("hello");
  });

  it("decodes plain and gzip trailer frames", async () => {
    const plain = encodeConnectFrame(new TextEncoder().encode("a"));
    const trailer = encodeConnectFrame(gzipSync(new TextEncoder().encode('{"error":{"code":"x","message":"bad"}}')), {
      endStream: true
    });
    trailer[0] = 3;
    const decoded = await collect(decodeConnectFrames([plain, trailer]));
    expect(new TextDecoder().decode(decoded[0]?.payload)).toBe("a");
    expect(decoded[1]?.endStream).toBe(true);
    expect(new TextDecoder().decode(decoded[1]?.payload)).toContain("bad");
  });

  it("rejects overlarge frame lengths", async () => {
    const frame = new Uint8Array(5);
    new DataView(frame.buffer).setUint32(1, MAX_CONNECT_FRAME_PAYLOAD + 1, false);
    await expect(collect(decodeConnectFrames([frame]))).rejects.toBeInstanceOf(DevinProtocolError);
  });
});
