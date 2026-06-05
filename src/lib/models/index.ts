import type { ModelProvider } from "./types";
import type { ProviderId } from "./providers";
import { OllamaProvider } from "./ollama";
import { CloudProvider } from "./cloud";

export * from "./types";
export * from "./providers";

/** Returns a provider instance for the given id. */
export function getProvider(id: ProviderId): ModelProvider {
  if (id === "ollama") return new OllamaProvider();
  return new CloudProvider(id);
}
