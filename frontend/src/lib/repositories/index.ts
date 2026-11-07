import { frontendConfig } from "@/config/frontend";
import { apiRepositories } from "@/lib/repositories/api";
import { mockRepositories } from "@/lib/repositories/mock";
import type { DataRepositories } from "@/lib/repositories/types";

/**
 * Mock data is only available in explicitly selected mock mode.
 * API mode always uses authenticated backend repositories.
 */
export function getRepositories(): DataRepositories {
  return frontendConfig.dataMode === "api" ? apiRepositories : mockRepositories;
}
