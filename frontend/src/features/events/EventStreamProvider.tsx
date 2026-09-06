"use client";

import { createContext, useContext } from "react";
import { useEventStream, type EventStreamState } from "@/hooks/useEventStream";

const Ctx = createContext<EventStreamState | null>(null);

export function EventStreamProvider({ children }: { children: React.ReactNode }) {
  const value = useEventStream();
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useSharedEventStream(): EventStreamState {
  const ctx = useContext(Ctx);
  if (!ctx) {
    throw new Error("useSharedEventStream must be used within EventStreamProvider");
  }
  return ctx;
}
