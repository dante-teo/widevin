import { loginDevin } from "./auth.js";
import { listDevinModels } from "./models.js";
import { createMemoryTokenStore } from "./token.js";
import { streamDevinChat } from "./chat.js";
import type { DevinChatRequest, DevinProvider, DevinProviderOptions } from "./types.js";

export const createDevinProvider = (options: DevinProviderOptions = {}): DevinProvider => {
  const tokenStore = options.tokenStore ?? createMemoryTokenStore();
  const fetchImpl = options.fetch ?? fetch;
  const getToken = async () => tokenStore.get();
  return {
    login: async () => {
      const token = await loginDevin({
        fetch: fetchImpl,
        ...(options.openBrowser ? { openBrowser: options.openBrowser } : {}),
        ...(options.appBaseUrl ? { appBaseUrl: options.appBaseUrl } : {}),
        ...(options.authBaseUrl ? { authBaseUrl: options.authBaseUrl } : {})
      });
      await tokenStore.set(token);
      return token;
    },
    setToken: (token) => tokenStore.set(token),
    clearToken: () => tokenStore.clear(),
    listModels: async () => {
      const token = await getToken();
      return listDevinModels({
        ...(token ? { token } : {}),
        ...(options.baseUrl ? { baseUrl: options.baseUrl } : {}),
        fetch: fetchImpl
      });
    },
    streamChat: async function* (request: DevinChatRequest) {
      const token = await getToken();
      yield* streamDevinChat({
        ...request,
        fetch: fetchImpl,
        ...(token ? { token } : {}),
        ...(options.baseUrl ? { baseUrl: options.baseUrl } : {}),
        ...(options.uuid ? { uuid: options.uuid } : {})
      });
    }
  };
};
