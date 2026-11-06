"use client";

import Link from "next/link";
import { useState } from "react";
import { useRouter } from "next/navigation";
import { frontendConfig } from "@/config/frontend";
import { login, register } from "@/lib/auth/session";

export default function LoginPage() {
  const router = useRouter();
  const [mode, setMode] = useState<"login" | "register">("login");
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setBusy(true);
    try {
      if (mode === "register") await register(email, displayName, password);
      else await login(email, password);
      router.replace("/journal");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Authentication failed.");
    } finally { setBusy(false); }
  }

  if (frontendConfig.dataMode !== "api") {
    return <section className="panel"><h1>EdgeNest sign in</h1><p>Set NEXT_PUBLIC_DATA_MODE=api and NEXT_PUBLIC_API_URL to enable Identity API authentication.</p><Link className="btn" href="/">Back to dashboard</Link></section>;
  }

  return <div style={{ maxWidth: 480, margin: "40px auto" }}>
    <section className="panel">
      <header className="page-header"><div><h1>{mode === "login" ? "Sign in" : "Create EdgeNest account"}</h1><p>Authenticate with the Rust Identity API.</p></div><span className="badge">API auth</span></header>
      <form onSubmit={submit} style={{ display: "grid", gap: 12 }}>
        {mode === "register" ? <label className="journal-field"><span>Display name</span><input required autoComplete="name" value={displayName} onChange={e => setDisplayName(e.target.value)} /></label> : null}
        <label className="journal-field"><span>Email</span><input required type="email" autoComplete="email" value={email} onChange={e => setEmail(e.target.value)} /></label>
        <label className="journal-field"><span>Password</span><input required type="password" autoComplete={mode === "login" ? "current-password" : "new-password"} minLength={12} value={password} onChange={e => setPassword(e.target.value)} /></label>
        {error ? <div className="error" role="alert">{error}</div> : null}
        <button className="btn primary" type="submit" disabled={busy}>{busy ? "Please wait…" : mode === "login" ? "Sign in" : "Create account"}</button>
      </form>
      <p className="metric-sub" style={{ marginTop: 12 }}>{mode === "login" ? "New to EdgeNest?" : "Already have an account?"}{" "}
        <button className="btn" type="button" onClick={() => { setMode(mode === "login" ? "register" : "login"); setError(null); }}>{mode === "login" ? "Register" : "Sign in"}</button>
      </p>
      <p className="metric-sub">Tokens are stored in this browser tab's sessionStorage and cleared on sign-out. This is not equivalent to HttpOnly-cookie/BFF protection against XSS; use a trusted HTTPS origin and review this auth-storage decision before production.</p>
    </section>
  </div>;
}
