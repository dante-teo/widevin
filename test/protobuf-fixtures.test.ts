import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { decodeProto, schemas } from "../src/devin/proto.js";

describe("cross-language protobuf fixtures", () => {
  it("decodes a Rust-generated model request in TypeScript", () => {
    const fixture = readFileSync(
      new URL("../rust/fixtures/protobuf/rust-models-request.base64", import.meta.url),
      "utf8"
    ).trim();
    const request = decodeProto(schemas.getCliModelConfigsRequest, Buffer.from(fixture, "base64"));

    expect(request.metadata).toMatchObject({
      apiKey: "devin-session-token$fixture",
      ideName: "windsurf",
      ideVersion: "3.2.23",
      extensionName: "windsurf",
      extensionVersion: "1.48.2",
      locale: "en"
    });
  });
});
