import { createHash, randomBytes, randomUUID } from "node:crypto";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import {
  DEVIN_APP_BASE_URL,
  DEVIN_AUTH_BASE_URL,
  DEVIN_OAUTH_CALLBACK_PATH,
  DEVIN_OAUTH_CALLBACK_PORT,
  DEVIN_OAUTH_TOKEN_PATH
} from "./constants.js";
import { DevinAuthError } from "./errors.js";
import type { FetchLike } from "./types.js";

export interface DevinLoginOptions {
  fetch?: FetchLike;
  openBrowser?: (url: string) => Promise<void> | void;
  appBaseUrl?: string;
  authBaseUrl?: string;
}

export interface PkcePair {
  verifier: string;
  challenge: string;
}

export const createPkcePair = (): PkcePair => {
  const verifier = base64Url(randomBytes(32));
  const challenge = base64Url(createHash("sha256").update(verifier).digest());
  return { verifier, challenge };
};

export const buildDevinAuthUrl = ({
  appBaseUrl = DEVIN_APP_BASE_URL,
  redirectUri,
  state,
  challenge
}: {
  appBaseUrl?: string;
  redirectUri: string;
  state: string;
  challenge: string;
}): string => {
  const params = new URLSearchParams({
    redirect_uri: redirectUri,
    state,
    prompt: "select_account",
    code_challenge: challenge,
    code_challenge_method: "S256"
  });
  return `${appBaseUrl.replace(/\/+$/, "")}/auth/cli/continue?${params.toString()}`;
};

export const exchangeDevinCliToken = async ({
  code,
  verifier,
  fetch: fetchImpl = fetch,
  authBaseUrl = DEVIN_AUTH_BASE_URL
}: {
  code: string;
  verifier: string;
  fetch?: FetchLike;
  authBaseUrl?: string;
}): Promise<string> => {
  const response = await fetchImpl(`${authBaseUrl.replace(/\/+$/, "")}${DEVIN_OAUTH_TOKEN_PATH}`, {
    method: "POST",
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json"
    },
    body: JSON.stringify({
      code,
      code_verifier: verifier
    })
  });
  if (!response.ok) {
    throw new DevinAuthError(`Devin CLI token exchange failed: ${response.status} ${await response.text()}`.trim());
  }
  const data = (await response.json()) as { token?: unknown };
  if (typeof data.token !== "string" || !data.token) {
    throw new DevinAuthError("Devin CLI token exchange returned an empty token");
  }
  return data.token;
};

export const validateDevinOAuthCallback = (url: URL, state: string): string => {
  if (url.pathname !== DEVIN_OAUTH_CALLBACK_PATH) {
    throw new DevinAuthError("Unexpected Devin OAuth callback path");
  }
  if (url.searchParams.get("state") !== state) {
    throw new DevinAuthError("Invalid Devin OAuth callback state");
  }
  const code = url.searchParams.get("code");
  if (!code) {
    throw new DevinAuthError("Missing Devin OAuth callback code");
  }
  return code;
};

export const loginDevin = async (options: DevinLoginOptions = {}): Promise<string> => {
  const pkce = createPkcePair();
  const state = randomUUID();
  const callback = await waitForOAuthCallback({ state });
  const url = buildDevinAuthUrl({
    redirectUri: callback.redirectUri,
    state,
    challenge: pkce.challenge,
    ...(options.appBaseUrl ? { appBaseUrl: options.appBaseUrl } : {})
  });
  await options.openBrowser?.(url);
  const code = await callback.code;
  return exchangeDevinCliToken({
    code,
    verifier: pkce.verifier,
    ...(options.fetch ? { fetch: options.fetch } : {}),
    ...(options.authBaseUrl ? { authBaseUrl: options.authBaseUrl } : {})
  });
};

const waitForOAuthCallback = async ({ state }: { state: string }): Promise<{ redirectUri: string; code: Promise<string> }> => {
  let resolveCode: (code: string) => void;
  let rejectCode: (error: Error) => void;
  const code = new Promise<string>((resolve, reject) => {
    resolveCode = resolve;
    rejectCode = reject;
  });

  const server = createServer((request, response) => {
    const url = new URL(request.url ?? "/", `http://${request.headers.host ?? "127.0.0.1"}`);
    const fail = (message: string) => {
      response.writeHead(400, { "content-type": "text/plain" });
      response.end(message);
      rejectCode(new DevinAuthError(message));
      server.close();
    };
    let value: string;
    try {
      value = validateDevinOAuthCallback(url, state);
    } catch (error) {
      fail(error instanceof Error ? error.message : "Invalid Devin OAuth callback");
      return;
    }
    response.writeHead(200, { "content-type": "text/plain" });
    response.end("Devin authentication complete. You can close this window.");
    resolveCode(value);
    server.close();
  });

  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(DEVIN_OAUTH_CALLBACK_PORT, "127.0.0.1", () => resolve());
  });
  const address = server.address() as AddressInfo;
  return { redirectUri: `http://127.0.0.1:${address.port}${DEVIN_OAUTH_CALLBACK_PATH}`, code };
};

const base64Url = (bytes: Uint8Array): string =>
  Buffer.from(bytes).toString("base64").replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/u, "");
