export type FrontendConfig = {
  appName: string;
  dataMode: "mock" | "api";
  eventMode: "mock" | "websocket" | "sse";
  environment: "paper" | "shadow" | "live";
  apiUrl: string;
  eventStreamUrl: string;
};

const configuredDataMode = process.env.NEXT_PUBLIC_DATA_MODE as "mock" | "api" | undefined;

export const frontendConfig: FrontendConfig = {
  appName: process.env.NEXT_PUBLIC_APP_NAME ?? "EdgeNest",
  // Production defaults to backend-backed data. Sample data is only enabled explicitly.
  dataMode: configuredDataMode ?? "api",
  eventMode:
    (process.env.NEXT_PUBLIC_EVENT_MODE as "mock" | "websocket" | "sse") ??
    "mock",
  environment:
    (process.env.NEXT_PUBLIC_ENVIRONMENT?.toLowerCase() as
      | "paper"
      | "shadow"
      | "live") ?? "paper",
  apiUrl: process.env.NEXT_PUBLIC_API_URL ?? "",
  eventStreamUrl: process.env.NEXT_PUBLIC_EVENT_STREAM_URL ?? "",
};

export function environmentLabel(
  env: FrontendConfig["environment"],
): "PAPER" | "SHADOW" | "LIVE" {
  return env.toUpperCase() as "PAPER" | "SHADOW" | "LIVE";
}
