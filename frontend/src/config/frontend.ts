export type FrontendConfig = {
  appName: string;
  environment: "paper" | "shadow" | "live";
  apiUrl: string;
};

export const frontendConfig: FrontendConfig = {
  appName: process.env.NEXT_PUBLIC_APP_NAME ?? "EdgeNest",
  environment:
    (process.env.NEXT_PUBLIC_ENVIRONMENT?.toLowerCase() as
      | "paper"
      | "shadow"
      | "live") ?? "paper",
  apiUrl: process.env.NEXT_PUBLIC_API_URL ?? "",
};

export function environmentLabel(
  env: FrontendConfig["environment"],
): "PAPER" | "SHADOW" | "LIVE" {
  return env.toUpperCase() as "PAPER" | "SHADOW" | "LIVE";
}
