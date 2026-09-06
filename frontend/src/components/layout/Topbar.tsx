"use client";

import { Bell, Menu, PanelLeft, Search, User } from "lucide-react";
import { environmentLabel, frontendConfig } from "@/config/frontend";

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

  return (
    <header className="topbar">
      <button
        type="button"
        className="icon-btn mobile-nav-btn"
        aria-label="Open navigation"
        onClick={onToggleMobile}
      >
        <Menu size={18} />
      </button>
      <button
        type="button"
        className="icon-btn"
        aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        onClick={onToggleCollapse}
      >
        <PanelLeft size={16} />
      </button>
      <div className="brand">
        <span className="brand-name">{frontendConfig.appName.toUpperCase()}</span>
        <span className="env-pill" title="Execution environment">
          {env}
        </span>
      </div>
      <div className="search" role="search">
        <Search size={14} aria-hidden />
        <span>Search accounts, strategies, instruments…</span>
        <kbd>⌘</kbd>
        <kbd>K</kbd>
      </div>
      <div className="topbar-right">
        <span aria-live="polite">
          <span className="status-dot ok" aria-hidden />
          SYSTEM HEALTHY
        </span>
        <span>
          <span
            className={`status-dot ${eventConnected ? "ok" : "off"}`}
            aria-hidden
          />
          EVENT STREAM
        </span>
        <button type="button" className="icon-btn" aria-label="Notifications">
          <Bell size={16} />
        </button>
        <button type="button" className="icon-btn" aria-label="Operator menu">
          <User size={16} />
        </button>
      </div>
    </header>
  );
}
