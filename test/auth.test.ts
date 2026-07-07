import { createHash } from "node:crypto";
import { describe, expect, it, vi } from "vitest";
import { buildDevinAuthUrl, createPkcePair, exchangeDevinCliToken, DevinAuthError } from "../src/index.js";
import { validateDevinOAuthCallback } from "../src/devin/auth.js";

describe("Devin auth", () => {
  it("generates PKCE S256 challenge", () => {
    const pair = createPkcePair();
    const expected = createHash("sha256").update(pair.verifier).digest("base64url");
    expect(pair.challenge).toBe(expected);
  });

  it("builds the Devin CLI OAuth URL", () => {
    const url = new URL(
      buildDevinAuthUrl({
        appBaseUrl: "https://app.example",
        redirectUri: "http://127.0.0.1:59653/callback",
        state: "state-1",
        challenge: "challenge-1"
      })
    );
    expect(url.toString()).toContain("https://app.example/auth/cli/continue?");
    expect(url.searchParams.get("redirect_uri")).toBe("http://127.0.0.1:59653/callback");
    expect(url.searchParams.get("state")).toBe("state-1");
    expect(url.searchParams.get("code_challenge")).toBe("challenge-1");
    expect(url.searchParams.get("code_challenge_method")).toBe("S256");
  });

  it("exchanges the code with JSON body and headers", async () => {
    const fetch = vi.fn(async () => new Response(JSON.stringify({ token: "raw-token" }), { status: 200 }));
    await expect(
      exchangeDevinCliToken({ code: "code-1", verifier: "verifier-1", authBaseUrl: "https://api.example", fetch })
    ).resolves.toBe("raw-token");
    expect(fetch).toHaveBeenCalledWith("https://api.example/auth/cli/token", {
      method: "POST",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json"
      },
      body: JSON.stringify({ code: "code-1", code_verifier: "verifier-1" })
    });
  });

  it("rejects a callback with the wrong state", () => {
    expect(() =>
      validateDevinOAuthCallback(new URL("http://127.0.0.1:59653/callback?state=wrong&code=code-1"), "state-1")
    ).toThrow(DevinAuthError);
  });

  it("accepts a successful callback code", () => {
    expect(validateDevinOAuthCallback(new URL("http://127.0.0.1:59653/callback?state=state-1&code=code-1"), "state-1")).toBe(
      "code-1"
    );
  });
});
