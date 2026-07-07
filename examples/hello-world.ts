/**
 * Manual smoke test for widevin.
 *
 * Run:
 *   pnpm run example:hello
 *   pnpm run example:hello -- "Explain what this library does in one sentence."
 *
 * First run opens your browser to sign in to Devin/Cascade. The resulting
 * token is cached in `.devin-token` at the repo root (gitignored), so later
 * runs skip the browser step. Set DEVIN_TOKEN to use an existing token
 * instead of logging in.
 *
 * This file imports from `../src/index.js` because it lives inside the
 * widevin repo itself. A real consumer would instead write:
 *
 *   import { createDevinProvider, createFileTokenStore } from "widevin";
 */
import { spawn } from "node:child_process";
import {
  createDevinProvider,
  createFileTokenStore,
  DevinApiError,
  DevinAuthError,
  DevinProtocolError
} from "../src/index.js";

const TOKEN_PATH = ".devin-token";

const openUrlInBrowser = (url: string): void => {
  console.error(`Open this URL to sign in to Devin: ${url}`);
  const [command, args] =
    process.platform === "darwin"
      ? (["open", [url]] as const)
      : process.platform === "win32"
        ? (["cmd", ["/c", "start", "", url]] as const)
        : (["xdg-open", [url]] as const);
  try {
    spawn(command, args, { stdio: "ignore", detached: true }).unref();
  } catch {
    console.error("Could not launch a browser automatically; open the URL above manually.");
  }
};

const main = async (): Promise<void> => {
  const message = process.argv.slice(2).join(" ") || "Say hello in one short sentence.";
  const tokenStore = createFileTokenStore(TOKEN_PATH);
  const devin = createDevinProvider({ tokenStore, openBrowser: openUrlInBrowser });

  if (process.env.DEVIN_TOKEN) {
    console.error("Using DEVIN_TOKEN from the environment.");
    await devin.setToken(process.env.DEVIN_TOKEN);
  } else if (await tokenStore.get()) {
    console.error(`Using cached token from ${TOKEN_PATH}`);
  } else {
    console.error("No cached token found, starting Devin login...");
    await devin.login();
    console.error(`Login complete. Token cached at ${TOKEN_PATH}`);
  }

  console.error("Fetching available models...");
  const models = await devin.listModels();
  const model = models[0];
  if (!model) throw new Error("Devin returned no available models");
  console.error(`Using model ${model.id} (${model.name})`);

  console.error(`Sending: ${message}`);
  console.error("---");

  let sawText = false;
  for await (const event of devin.streamChat({
    model: model.id,
    systemPrompt: ["You are concise."],
    messages: [{ role: "user", content: message }]
  })) {
    if (event.type === "text_delta") {
      sawText = true;
      process.stdout.write(event.delta);
    } else if (event.type === "usage") {
      console.error(
        `\n[usage] input=${event.inputTokens} output=${event.outputTokens} cacheRead=${event.cacheReadTokens} cacheWrite=${event.cacheWriteTokens}`
      );
    } else if (event.type === "done") {
      if (sawText) process.stdout.write("\n");
      console.error(`[done] reason=${event.reason}`);
    }
  }
};

main().catch((error: unknown) => {
  if (error instanceof DevinAuthError) {
    console.error(`Devin login failed: ${error.message}`);
  } else if (error instanceof DevinApiError) {
    console.error(`Devin API request failed (${error.status}): ${error.message}`);
  } else if (error instanceof DevinProtocolError) {
    console.error(`Devin protocol error: ${error.message}`);
  } else {
    console.error(error);
  }
  process.exitCode = 1;
});
