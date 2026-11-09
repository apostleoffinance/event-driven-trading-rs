"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Activity,
  Briefcase,
  BookOpen,
  Calculator,
  ChartNoAxesCombined,
  CandlestickChart,
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
    title: "OVERVIEW",
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
    title: "JOURNAL",
    items: [
      { href: "/journal", label: "Journal", icon: BookOpen },
      { href: "/calculator", label: "Calculator", icon: Calculator },
      { href: "/analytics", label: "Analytics Lab", icon: ChartNoAxesCombined },
    ],
  },
  {
    title: "ACCOUNTS",
    items: [
      { href: "/accounts", label: "Accounts", icon: Wallet },
      { href: "/portfolio", label: "Portfolio", icon: Activity },
    ],
  },
  {
    title: "RISK",
    items: [{ href: "/risk", label: "Risk Center", icon: Shield }, { href: "/risk/policies", label: "Account Policies", icon: SlidersHorizontal }],
  },
  {
    title: "TOOLS",
    items: [
      { href: "/market-data", label: "Market Data", icon: CandlestickChart },
      { href: "/events", label: "Activity", icon: Radio },
      { href: "/reconciliation", label: "Reconciliation", icon: Scale },
      { href: "/system", label: "Connections & Status", icon: Server },
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
