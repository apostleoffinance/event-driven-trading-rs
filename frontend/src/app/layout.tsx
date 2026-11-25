import type { Metadata } from "next";
import { AppShell } from "@/components/layout/AppShell";
import "@/styles/globals.css";
import "@/styles/design-tokens.css";
import "@/styles/brand-assets.css";
import "@/styles/workspace-settings.css";

export const metadata: Metadata = {
  title: "EdgeNest | Trading Infrastructure",
  description:
    "A unified trading workspace for position sizing, journaling, performance analytics and risk oversight.",
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" data-theme="dark" suppressHydrationWarning>
      <body>
        <script
          dangerouslySetInnerHTML={{
            __html: `(function(){try{var t=localStorage.getItem("quant-os-theme");document.documentElement.setAttribute("data-theme",t==="light"?"light":"dark");}catch(e){document.documentElement.setAttribute("data-theme","dark");}})();`,
          }}
        />
        <AppShell>{children}</AppShell>
      </body>
    </html>
  );
}
