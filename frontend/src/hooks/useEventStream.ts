"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { frontendConfig } from "@/config/frontend";
import { mockEvents } from "@/mocks/events";
import type { TradingEvent } from "@/types/domain";

export type EventStreamState = {
  events: TradingEvent[];
  connected: boolean;
  push: (event: TradingEvent) => void;
  reset: () => void;
};

/**
 * Mock event stream with optional live tick injection for F6 simulation.
 * Swap for WebSocket/SSE without changing consumers.
 */
export function useEventStream(limit = 40): EventStreamState {
  const [events, setEvents] = useState<TradingEvent[]>(() => [...mockEvents]);
  const [connected, setConnected] = useState(false);
  const seq = useRef(100);

  useEffect(() => {
    if (frontendConfig.eventMode !== "mock") {
      setConnected(false);
      return;
    }
    setConnected(true);
    const id = window.setInterval(() => {
      seq.current += 1;
      const tick: TradingEvent = {
        id: `evt-live-${seq.current}`,
        timestamp: new Date().toISOString(),
        type: "MarketDataReceived",
        instrumentId: "EURUSD",
        detail: `Mock tick ${seq.current}`,
        status: "ok",
      };
      setEvents((prev) => [tick, ...prev].slice(0, limit));
    }, 8000);
    return () => window.clearInterval(id);
  }, [limit]);

  const push = useCallback(
    (event: TradingEvent) => {
      setEvents((prev) => [event, ...prev].slice(0, limit));
    },
    [limit],
  );

  const reset = useCallback(() => {
    setEvents([...mockEvents]);
  }, []);

  return { events, connected, push, reset };
}
