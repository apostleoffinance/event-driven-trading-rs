"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { usePathname } from "next/navigation";
import { hasSession } from "@/lib/auth/session";
import { apiRequest } from "@/lib/api/client";

export type Organization = {
  id: string;
  name: string;
  status: "ACTIVE" | "SUSPENDED" | "CLOSED";
  membershipRole: "OWNER" | "ADMIN" | "TRADER" | "RISK_MANAGER" | "VIEWER";
};

type OrganizationContextValue = {
  organizations: Organization[];
  activeOrganization: Organization | null;
  activeOrganizationId: string | null;
  loading: boolean;
  error: string | null;
  selectOrganization: (id: string) => void;
  refreshOrganizations: () => Promise<void>;
  createOrganization: (name: string) => Promise<void>;
};

const STORAGE_KEY = "edgenest.organization.active.v1";
const OrganizationContext = createContext<OrganizationContextValue | null>(null);

export function OrganizationProvider({ children }: { children: React.ReactNode }) {
  const queryClient = useQueryClient();
  const pathname = usePathname();
  const [organizations, setOrganizations] = useState<Organization[]>([]);
  const [activeOrganizationId, setActiveOrganizationId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refreshOrganizations = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await apiRequest<{ data: Organization[] }>("/api/v1/organizations");
      const available = response.data.filter((org) => org.status === "ACTIVE");
      setOrganizations(available);
      const stored = window.sessionStorage.getItem(STORAGE_KEY);
      const storedIsValid = stored && available.some((org) => org.id === stored);
      const nextId = storedIsValid ? stored : available.length === 1 ? available[0].id : null;
      setActiveOrganizationId(nextId);
      if (nextId) window.sessionStorage.setItem(STORAGE_KEY, nextId);
      else window.sessionStorage.removeItem(STORAGE_KEY);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "We couldn't load your organizations. Please try again.");
      setOrganizations([]);
      setActiveOrganizationId(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (hasSession()) void refreshOrganizations();
    else { setOrganizations([]); setActiveOrganizationId(null); setError(null); setLoading(false); window.sessionStorage.removeItem(STORAGE_KEY); }
  }, [pathname, refreshOrganizations]);

  const createOrganization = useCallback(async (name: string) => {
    const cleanedName = name.trim();
    if (!cleanedName || cleanedName.length > 120) {
      throw new Error("Workspace name must contain between 1 and 120 characters.");
    }
    const response = await apiRequest<{ data: Organization }>("/api/v1/organizations", {
      method: "POST",
      body: JSON.stringify({ name: cleanedName }),
    });
    const created = response.data;
    if (!created?.id || created.status !== "ACTIVE") {
      throw new Error("The workspace was created, but the API returned an invalid workspace record. Refresh to verify it.");
    }
    setOrganizations(current => [...current.filter(org => org.id !== created.id), created]);
    setActiveOrganizationId(created.id);
    window.sessionStorage.setItem(STORAGE_KEY, created.id);
    setError(null);
    queryClient.clear();
  }, [queryClient]);

  const selectOrganization = useCallback((id: string) => {
    if (!organizations.some((org) => org.id === id)) return;
    if (id === activeOrganizationId) return;
    window.sessionStorage.setItem(STORAGE_KEY, id);
    setActiveOrganizationId(id);
    queryClient.clear();
  }, [activeOrganizationId, organizations, queryClient]);

  const activeOrganization = organizations.find((org) => org.id === activeOrganizationId) ?? null;
  const value = useMemo(() => ({
    organizations, activeOrganization, activeOrganizationId, loading, error,
    selectOrganization, refreshOrganizations, createOrganization,
  }), [organizations, activeOrganization, activeOrganizationId, loading, error, selectOrganization, refreshOrganizations, createOrganization]);

  return <OrganizationContext.Provider value={value}>{children}</OrganizationContext.Provider>;
}

export function useOrganization() {
  const context = useContext(OrganizationContext);
  if (!context) throw new Error("useOrganization must be used within OrganizationProvider");
  return context;
}
