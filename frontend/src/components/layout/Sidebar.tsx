"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Activity,
  Briefcase,
  Crosshair,
  LayoutDashboard,
  ListOrdered,
  Radio,
  Scale,
  Server,
  Shield,
  Wallet,
} from "lucide-react";

const sections = [
  {
    title: "COMMAND",
    items: [{ href: "/", label: "Dashboard", icon: LayoutDashboard }],
  },
  {
    title: "TRADING",
    items: [
      { href: "/strategies", label: "Strategies", icon: Crosshair },
      { href: "/positions", label: "Positions", icon: Briefcase },
      { href: "/orders", label: "Orders", icon: ListOrdered },
    ],
  },
  {
    title: "CAPITAL",
    items: [
      { href: "/accounts", label: "Accounts", icon: Wallet },
      { href: "/portfolio", label: "Portfolio", icon: Activity },
    ],
  },
  {
    title: "RISK",
    items: [{ href: "/risk", label: "Risk Center", icon: Shield }],
  },
  {
    title: "SYSTEM",
    items: [
      { href: "/events", label: "Event Stream", icon: Radio },
      { href: "/reconciliation", label: "Reconcile", icon: Scale },
      { href: "/system", label: "System", icon: Server },
    ],
  },
];

export function Sidebar({
  collapsed,
  mobileOpen,
  onNavigate,
}: {
  collapsed: boolean;
  mobileOpen: boolean;
  onNavigate?: () => void;
}) {
  const pathname = usePathname();

  return (
    <aside
      className={`sidebar${mobileOpen ? " open" : ""}`}
      aria-label="Primary"
    >
      {sections.map((section) => (
        <div key={section.title}>
          {!collapsed && <div className="nav-section">{section.title}</div>}
          {section.items.map((item) => {
            const Icon = item.icon;
            const active =
              item.href === "/"
                ? pathname === "/"
                : pathname.startsWith(item.href);
            return (
              <Link
                key={item.href}
                href={item.href}
                className={`nav-link${active ? " active" : ""}`}
                aria-current={active ? "page" : undefined}
                title={item.label}
                onClick={onNavigate}
              >
                <Icon size={16} aria-hidden />
                {!collapsed && <span>{item.label}</span>}
              </Link>
            );
          })}
        </div>
      ))}
    </aside>
  );
}
