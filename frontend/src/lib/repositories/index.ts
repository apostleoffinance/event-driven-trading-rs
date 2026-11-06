import { frontendConfig } from "@/config/frontend";
import { mockRepositories } from "@/lib/repositories/mock";
import type { DataRepositories } from "@/lib/repositories/types";

/**
 * Trading dashboard repositories are still mock-only until the Rust trading
 * read APIs are implemented. API mode must never quietly display demo data.
 */
export function getRepositories(): DataRepositories {
  if (frontendConfig.dataMode === "api") {
    throw new Error(
      "Trading dashboard API repositories are not implemented yet. Mock data is intentionally disabled in API mode.",
    );
  }
  return mockRepositories;
}
