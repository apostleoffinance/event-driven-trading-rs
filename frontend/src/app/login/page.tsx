"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { login, register, restoreSession } from "@/lib/auth/session";
import { Eye, EyeOff } from "lucide-react";

export default function LoginPage() {
  const router = useRouter();
  const [mode, setMode] = useState<"login" | "register">("login");
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);

  useEffect(() => {
    let active = true;
    void restoreSession().then(user => {
      if (active && user) router.replace("/");
    });
    return () => { active = false; };
  }, [router]);
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

  return <div style={{ maxWidth: 480, margin: "40px auto" }}>
    <section className="panel">
      <header className="page-header"><div><h1>{mode === "login" ? "Welcome back to EdgeNest" : "Create your EdgeNest account"}</h1><p>{mode === "login" ? "Sign in to manage your trading accounts and journal." : "Create an account to get started with EdgeNest."}</p></div></header>
      <form onSubmit={submit} style={{ display: "grid", gap: 12 }}>
        {mode === "register" ? <label className="journal-field"><span>Display name</span><input required autoComplete="name" value={displayName} onChange={e => setDisplayName(e.target.value)} /></label> : null}
        <label className="journal-field"><span>Email</span><input required type="email" autoComplete="email" value={email} onChange={e => setEmail(e.target.value)} /></label>
        <label className="journal-field auth-password-field"><span>Password</span><div className="auth-password-control"><input required type={showPassword ? "text" : "password"} autoComplete={mode === "login" ? "current-password" : "new-password"} minLength={12} value={password} onChange={e => setPassword(e.target.value)} /><button className="auth-password-toggle" type="button" aria-label={showPassword ? "Hide password" : "Show password"} aria-pressed={showPassword} onClick={() => setShowPassword(value => !value)}>{showPassword ? <EyeOff size={17} aria-hidden /> : <Eye size={17} aria-hidden />}</button></div></label>
        {error ? <div className="error" role="alert">{error}</div> : null}
        <button className="btn primary" type="submit" disabled={busy}>{busy ? "Please wait…" : mode === "login" ? "Sign in" : "Create account"}</button>
      </form>
      <p className="auth-switch">{mode === "login" ? "New to EdgeNest?" : "Already have an account?"}{" "}
        <button className="btn" type="button" onClick={() => { setMode(mode === "login" ? "register" : "login"); setError(null); }}>{mode === "login" ? "Register" : "Sign in"}</button>
      </p>
      
    </section>
  </div>;
}
