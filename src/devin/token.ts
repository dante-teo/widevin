import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { DEVIN_SESSION_TOKEN_PREFIX } from "./constants.js";
import type { TokenStore } from "./types.js";

export const createMemoryTokenStore = (initialToken?: string): TokenStore => {
  let token = initialToken;
  return {
    get: async () => token,
    set: async (nextToken) => {
      token = nextToken;
    },
    clear: async () => {
      token = undefined;
    }
  };
};

export const createFileTokenStore = (path: string): TokenStore => ({
  get: async () =>
    readFile(path, "utf8")
      .then((value) => value.trim() || undefined)
      .catch((error: NodeJS.ErrnoException) => {
        if (error.code === "ENOENT") return undefined;
        throw error;
      }),
  set: async (token) => {
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, token, { encoding: "utf8", mode: 0o600 });
  },
  clear: async () => {
    await rm(path, { force: true });
  }
});

export const normalizeDevinSessionToken = (token: string | undefined): string =>
  token ? (token.startsWith(DEVIN_SESSION_TOKEN_PREFIX) ? token : `${DEVIN_SESSION_TOKEN_PREFIX}${token}`) : "";
