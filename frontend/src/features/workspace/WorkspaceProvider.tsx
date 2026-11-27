"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { hasSession } from "@/lib/auth/session";
import { apiRequest } from "@/lib/api/client";
import { useOrganization } from "@/features/organization/OrganizationProvider";

export type Workspace = {
  id: string;
  organizationId: string;
  name: string;
  slug: string;
  status: "ACTIVE" | "ARCHIVED";
  isDefault: boolean;
  revision: number;
};
type WorkspaceContextValue = {
  workspaces: Workspace[];
  activeWorkspace: Workspace | null;
  activeWorkspaceId: string | null;
  loading: boolean;
  error: string | null;
  selectWorkspace: (id: string) => void;
  refreshWorkspaces: () => Promise<void>;
};
const STORAGE_KEY = "edgenest.workspace.active.v1";
const WorkspaceContext = createContext<WorkspaceContextValue | null>(null);

export function WorkspaceProvider({ children }: { children: React.ReactNode }) {
  const queryClient = useQueryClient();
  const { activeOrganizationId } = useOrganization();
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refreshWorkspaces = useCallback(async () => {
    if (!activeOrganizationId || !hasSession()) {
      setWorkspaces([]);
      setActiveWorkspaceId(null);
      if (typeof window !== "undefined") window.sessionStorage.removeItem(STORAGE_KEY);
      return;
    }
    setWorkspaces([]);
    setActiveWorkspaceId(null);
    setLoading(true);
    setError(null);
    try {
      const response = await apiRequest<{ data: Workspace[] }>(
        `/api/v1/organizations/${encodeURIComponent(activeOrganizationId)}/workspaces`,
        { headers: { "X-Organization-Id": activeOrganizationId } },
      );
      const available = (response.data ?? []).filter((workspace) => workspace.status === "ACTIVE");
      setWorkspaces(available);
      const stored = window.sessionStorage.getItem(STORAGE_KEY);
      const selected = stored && available.some((workspace) => workspace.id === stored)
        ? stored
        : available.length === 1 ? available[0].id : null;
      setActiveWorkspaceId(selected);
      if (selected) window.sessionStorage.setItem(STORAGE_KEY, selected);
      else window.sessionStorage.removeItem(STORAGE_KEY);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Unable to load accessible workspaces.");
      setWorkspaces([]);
      setActiveWorkspaceId(null);
      window.sessionStorage.removeItem(STORAGE_KEY);
    } finally {
      setLoading(false);
    }
  }, [activeOrganizationId]);

  useEffect(() => {
    if (hasSession() && activeOrganizationId) void refreshWorkspaces();
    else {
      setWorkspaces([]);
      setActiveWorkspaceId(null);
      if (typeof window !== "undefined") window.sessionStorage.removeItem(STORAGE_KEY);
    }
  }, [activeOrganizationId, refreshWorkspaces]);

  const selectWorkspace = useCallback((id: string) => {
    if (!workspaces.some((workspace) => workspace.id === id) || id === activeWorkspaceId) return;
    window.sessionStorage.setItem(STORAGE_KEY, id);
    setActiveWorkspaceId(id);
    queryClient.clear();
  }, [activeWorkspaceId, workspaces, queryClient]);

  const activeWorkspace = workspaces.find((workspace) => workspace.id === activeWorkspaceId) ?? null;
  const value = useMemo(() => ({
    workspaces, activeWorkspace, activeWorkspaceId, loading, error, selectWorkspace, refreshWorkspaces,
  }), [workspaces, activeWorkspace, activeWorkspaceId, loading, error, selectWorkspace, refreshWorkspaces]);

  return <WorkspaceContext.Provider value={value}>{children}</WorkspaceContext.Provider>;
}

export function useWorkspace() {
  const context = useContext(WorkspaceContext);
  if (!context) throw new Error("useWorkspace must be used within WorkspaceProvider");
  return context;
}
