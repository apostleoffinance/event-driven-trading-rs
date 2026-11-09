import { apiRepositories } from "@/lib/repositories/api";

/** Dashboard data is always read from the authenticated backend. */
export function getRepositories() {
  return apiRepositories;
}
