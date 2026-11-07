"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { Bell, ChevronDown, LogOut, Menu, Moon, PanelLeft, Search, Sun, User } from "lucide-react";
import { environmentLabel, frontendConfig } from "@/config/frontend";
import { apiBaseUrl } from "@/lib/api/client";
import { useTheme } from "@/features/theme/ThemeProvider";
import { getSessionUser, hasSession, logout } from "@/lib/auth/session";
import { useTradingEnvironment, type TradingEnvironment } from "@/features/environment/EnvironmentProvider";

export function Topbar({
  collapsed,
  eventConnected,
  onToggleCollapse,
  onToggleMobile,
}: {
  collapsed: boolean;
  eventConnected: boolean;
  onToggleCollapse: () => void;
  onToggleMobile: () => void;
}) {
  const env = environmentLabel(frontendConfig.environment);
  const { environment, setEnvironment } = useTradingEnvironment();
  const { theme, toggleTheme } = useTheme();
  const pathname = usePathname();
  const router = useRouter();
  const accountRef = useRef<HTMLDivElement>(null);
  const [accountOpen, setAccountOpen] = useState(false);
  const [authenticated, setAuthenticated] = useState(false);
  const [userEmail, setUserEmail] = useState<string | null>(null);
  const [signingOut, setSigningOut] = useState(false);
  const [backendStatus, setBackendStatus] = useState<"checking" | "connected" | "unavailable">("checking");

  // Auth state is stored in sessionStorage, so sync it after client-side route
  // changes (e.g. returning from login/register) rather than during SSR.
  useEffect(() => {
    setAuthenticated(hasSession());
    setUserEmail(getSessionUser()?.email ?? null);
    setAccountOpen(false);
  }, [pathname]);

  // Close the account menu on outside click or Escape.
  useEffect(() => {
    if (!accountOpen) return;
    const onPointerDown = (event: PointerEvent) => {
      if (accountRef.current && !accountRef.current.contains(event.target as Node)) {
        setAccountOpen(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setAccountOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [accountOpen]);

  useEffect(() => {
    if (frontendConfig.dataMode !== "api") {
      setBackendStatus("checking");
      return;
    }
    let active = true;
    const check = async () => {
      try {
        const origin = apiBaseUrl();
        const response = await fetch(`${origin}/healthz`, { cache: "no-store", headers: { Accept: "text/plain" } });
        if (active) setBackendStatus(response.ok ? "connected" : "unavailable");
      } catch {
        if (active) setBackendStatus("unavailable");
      }
    };
    void check();
    const id = window.setInterval(() => void check(), 30000);
    return () => { active = false; window.clearInterval(id); };
  }, []);

  async function handleSignOut() {
    if (signingOut) return;
    setSigningOut(true);
    setAccountOpen(false);
    try {
      await logout();
    } catch {
      // logout() clears local credentials in its finally block even if the API
      // is unreachable. Do not leave the browser appearing authenticated.
    } finally {
      setAuthenticated(false);
      setUserEmail(null);
      setSigningOut(false);
      router.replace("/login");
      router.refresh();
    }
  }

  return (
    <header className="topbar">
      <button type="button" className="icon-btn mobile-nav-btn" aria-label="Open navigation" onClick={onToggleMobile}><Menu size={18} /></button>
      <button type="button" className="icon-btn" aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"} onClick={onToggleCollapse}><PanelLeft size={16} /></button>
      <div className="brand">
        <span className="brand-name">{frontendConfig.appName.toUpperCase()}</span>
        <span className={`env-pill${environment === "LIVE" ? " live" : environment === "DEMO" ? " shadow" : ""}`} title="Selected trading data environment">{environment}</span>
      </div>
      <div className="search" role="search"><Search size={14} aria-hidden /><span>Search accounts, strategies, instruments…</span><kbd>⌘</kbd><kbd>K</kbd></div>
      <div className="topbar-right">
        <div className="environment-switcher" role="group" aria-label="Trading environment">
          {(["LIVE", "DEMO", "PAPER"] as TradingEnvironment[]).map((item) => (
            <button key={item} type="button" className={`environment-option${environment === item ? " active" : ""}${item === "LIVE" ? " live" : ""}`} aria-pressed={environment === item} onClick={() => setEnvironment(item)} title={`Show ${item.toLowerCase()} trading data`}>{item}</button>
          ))}
        </div>
        <span aria-live="polite"><span className={`status-dot ${frontendConfig.dataMode === "mock" ? "warn" : backendStatus === "connected" ? "ok" : backendStatus === "checking" ? "warn" : "off"}`} aria-hidden />{frontendConfig.dataMode === "mock" ? "DEMO MODE" : backendStatus === "connected" ? "SERVICE CONNECTED" : backendStatus === "checking" ? "CONNECTING" : "CONNECTION ISSUE"}</span>
        <span aria-live="polite"><span className={`status-dot ${eventConnected ? "ok" : "off"}`} aria-hidden />{frontendConfig.dataMode === "mock" ? "SIMULATION EVENTS" : eventConnected ? "LIVE UPDATES" : "UPDATES PAUSED"}</span>
        <button type="button" className="icon-btn" aria-label={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"} title={theme === "dark" ? "Light mode" : "Dark mode"} onClick={toggleTheme}>{theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}</button>
        <button type="button" className="icon-btn" aria-label="Notifications" title="Notifications"><Bell size={16} /></button>
        <div className="account-menu" ref={accountRef}>
          <button
            type="button"
            className="icon-btn account-trigger"
            aria-label="Account menu"
            aria-haspopup="menu"
            aria-expanded={accountOpen}
            title={authenticated ? userEmail ?? "Account menu" : "Sign in / account"}
            onClick={() => setAccountOpen((open) => !open)}
          >
            <User size={16} />
            <ChevronDown size={12} aria-hidden />
          </button>
          {accountOpen && (
            <div className="account-dropdown" role="menu" aria-label="Account actions">
              {authenticated ? (
                <>
                  <div className="account-menu-user">
                    <span className="account-menu-label">Signed in</span>
                    <span className="account-menu-email">{userEmail ?? "EdgeNest user"}</span>
                  </div>
                  <div className="account-menu-divider" />
                  <button type="button" role="menuitem" className="account-menu-item" onClick={handleSignOut} disabled={signingOut}>
                    <LogOut size={15} aria-hidden />
                    {signingOut ? "Signing out…" : "Sign out"}
                  </button>
                </>
              ) : (
                <>
                  <Link href="/login" role="menuitem" className="account-menu-item" onClick={() => setAccountOpen(false)}>Sign in</Link>
                  <Link href="/login" role="menuitem" className="account-menu-item" onClick={() => setAccountOpen(false)}>Register (on sign-in page)</Link>
                </>
              )}
            </div>
          )}
        </div>
      </div>
    </header>
  );
}
