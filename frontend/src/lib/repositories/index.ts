import { frontendConfig } from "@/config/frontend";
import { mockRepositories } from "@/lib/repositories/mock";
import type { DataRepositories } from "@/lib/repositories/types";

/**
 * Repository factory. API mode is a stub until Rust REST exists —
 * falls back to mock so the UI never hard-codes data sources.
 */
export function getRepositories(): DataRepositories {
  if (frontendConfig.dataMode === "api" && frontendConfig.apiUrl) {
    // Future: return apiRepositories(frontendConfig.apiUrl)
    console.warn(
      "[Quant OS] API mode requested but not wired; using mock repositories.",
    );
  }
  return mockRepositories;
}
