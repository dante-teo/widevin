import { describe, expect, it } from "vitest";
import {
  parsePossiblyCompleteJson,
  parsePossiblyCompleteJsonThrottled,
  parseStreamingJson,
  STREAMING_JSON_PARSE_MIN_GROWTH
} from "../src/devin/json.js";

describe("JSON parsing", () => {
  it("preserves each parser's fallback behavior without replacing valid null", () => {
    expect(parseStreamingJson(undefined)).toEqual({});
    expect(parseStreamingJson("{")).toEqual({});
    expect(parseStreamingJson("null")).toBeNull();
    expect(parsePossiblyCompleteJson("{")).toBeUndefined();
    expect(parsePossiblyCompleteJson("null")).toBeNull();
  });
});

describe("parsePossiblyCompleteJsonThrottled", () => {
  it("attempts the first non-empty buffer immediately", () => {
    expect(parsePossiblyCompleteJsonThrottled('{"q":', 0)).toEqual({
      value: undefined,
      attemptedLength: 5
    });
  });

  it("requires 256 characters of growth before attempting another parse", () => {
    const initial = '{"q":';
    const belowThreshold = `${initial}${" ".repeat(STREAMING_JSON_PARSE_MIN_GROWTH - 1)}`;
    const atThreshold = `${initial}${" ".repeat(STREAMING_JSON_PARSE_MIN_GROWTH)}`;

    expect(parsePossiblyCompleteJsonThrottled(belowThreshold, initial.length)).toBeNull();
    expect(parsePossiblyCompleteJsonThrottled(atThreshold, initial.length)).toEqual({
      value: undefined,
      attemptedLength: atThreshold.length
    });
  });
});
