"use client";

import { useCallback, useEffect, useState } from "react";
import { getRepositories } from "@/lib/repositories";
import type { TradingEvent } from "@/types/domain";

export type EventStreamState = {
  events: TradingEvent[];
  connected: boolean;
  push: (event: TradingEvent) => void;
  reset: () => void;
};

/** Refreshes the authenticated event history and never fabricates trading events. */
export function useEventStream(limit = 40): EventStreamState {
  const [events, setEvents] = useState<TradingEvent[]>([]);
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    let active = true;
    const refresh = async () => {
      try {
        const latest = await getRepositories().events.listEvents(limit);
        if (!active) return;
        setEvents(latest.slice(0, limit));
        setConnected(true);
      } catch {
        if (active) setConnected(false);
      }
    };
    void refresh();
    const id = window.setInterval(() => void refresh(), 10000);
    return () => {
      active = false;
      window.clearInterval(id);
    };
  }, [limit]);

  const push = useCallback((event: TradingEvent) => {
    setEvents((prev) => [event, ...prev].slice(0, limit));
  }, [limit]);

  const reset = useCallback(() => setEvents([]), []);

  return { events, connected, push, reset };
}
