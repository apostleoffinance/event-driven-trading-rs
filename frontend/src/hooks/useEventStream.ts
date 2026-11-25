"use client";

import { useCallback, useEffect, useState } from "react";
import { getRepositories } from "@/lib/repositories";
import type { TradingEvent } from "@/types/domain";
import { useWorkspace } from "@/features/workspace/WorkspaceProvider";
import { useOrganization } from "@/features/organization/OrganizationProvider";

export type EventStreamState = {
  events: TradingEvent[];
  connected: boolean;
  push: (event: TradingEvent) => void;
  reset: () => void;
};

/** Refreshes the authenticated event history and never fabricates trading events. */
export function useEventStream(limit = 50) {
  const [eventState, setEventState] = useState<{
    scope: string;
    events: TradingEvent[];
    connected: boolean;
  }>({ scope: "", events: [], connected: false });
  const { activeWorkspaceId } = useWorkspace();
  const { activeOrganizationId } = useOrganization();
  const scopeKey = `${activeOrganizationId ?? ""}:${activeWorkspaceId ?? ""}`;
  const events = eventState.scope === scopeKey ? eventState.events : [];
  const connected = eventState.scope === scopeKey && eventState.connected;
  const push = useCallback((event: TradingEvent) => {
    if (!activeOrganizationId || !activeWorkspaceId) return;
    setEventState((current) => ({
      scope: scopeKey,
      events: [event, ...(current.scope === scopeKey ? current.events : [])].slice(0, limit),
      connected: true,
    }));
  }, [activeOrganizationId, activeWorkspaceId, limit, scopeKey]);
  const reset = useCallback(() => {
    setEventState({ scope: scopeKey, events: [], connected: false });
  }, [scopeKey]);

  useEffect(() => {
    let active = true;
    if (!activeOrganizationId || !activeWorkspaceId) {
      return () => {
        active = false;
      };
    }
    const refresh = async () => {
      try {
        const latest = await getRepositories().events.listEvents(limit);
        if (active) {
          setEventState({ scope: scopeKey, events: latest, connected: true });
        }
      } catch {
        if (active) {
          setEventState({ scope: scopeKey, events: [], connected: false });
        }
      }
    };
    void refresh();
    const timer = window.setInterval(refresh, 10_000);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [limit, activeOrganizationId, activeWorkspaceId, scopeKey]);

  return { events, connected, push, reset };
}
