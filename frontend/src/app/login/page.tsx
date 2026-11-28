"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Eye, EyeOff } from "lucide-react";
import { login, register, restoreSession } from "@/lib/auth/session";

export default function LoginPage() {
  const router = useRouter();
  const [mode, setMode] = useState<"login" | "register">("login");
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void restoreSession().then(user => {
      if (active && user) router.replace("/");
    });
    return () => { active = false; };
  }, [router]);

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setBusy(true);
    try {
      if (mode === "register") await register(email, displayName, password);
      else await login(email, password);
      router.replace("/");
      router.refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Authentication failed.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="auth-layout">
      <section className="auth-card" aria-labelledby="auth-title">
        <div className="auth-brand">
          <span className="auth-brand-mark">E</span>
          <span>EDGENEST</span>
          <span className="env-pill">PAPER</span>
        </div>
        <header className="auth-heading">
          <span className="auth-eyebrow">SECURE WORKSPACE ACCESS</span>
          <h1 id="auth-title">{mode === "login" ? "Welcome back" : "Create your account"}</h1>
          <p>{mode === "login" ? "Sign in to access your trading workspace." : "Create an account to get started with EdgeNest."}</p>
        </header>
        <form onSubmit={submit}>
          {mode === "register" ? (
            <label className="journal-field">
              <span>Display name</span>
              <input required autoComplete="name" value={displayName} onChange={event => setDisplayName(event.target.value)} />
            </label>
          ) : null}
          <label className="journal-field">
            <span>Email</span>
            <input required type="email" autoComplete="email" value={email} onChange={event => setEmail(event.target.value)} />
          </label>
          <label className="journal-field auth-password-field">
            <span>Password</span>
            <div className="auth-password-control">
              <input
                id="edgenest-password"
                name="password"
                aria-label="Password"
                required
                type={showPassword ? "text" : "password"}
                autoComplete={mode === "login" ? "current-password" : "new-password"}
                minLength={12}
                value={password}
                onChange={event => setPassword(event.target.value)}
              />
              <button
                className="auth-password-toggle"
                type="button"
                aria-label={showPassword ? "Hide password" : "Show password"}
                aria-pressed={showPassword}
                title={showPassword ? "Hide password" : "Show password"}
                onClick={() => setShowPassword(value => !value)}
              >
                {showPassword ? <EyeOff size={17} aria-hidden /> : <Eye size={17} aria-hidden />}
              </button>
            </div>
          </label>
          {error ? <div className="error" role="alert">{error}</div> : null}
          <button className="btn primary" type="submit" disabled={busy}>
            {busy ? "Please wait…" : mode === "login" ? "Sign in" : "Create account"}
          </button>
        </form>
        <p className="auth-switch">
          {mode === "login" ? "New to EdgeNest?" : "Already have an account?"}{" "}
          <button
            className="btn"
            type="button"
            onClick={() => {
              setMode(value => value === "login" ? "register" : "login");
              setError(null);
              setShowPassword(false);
            }}
          >
            {mode === "login" ? "Register" : "Sign in"}
          </button>
        </p>
        <p className="auth-footnote">Your session is protected with secure, HTTP-only authentication cookies.</p>
      </section>
    </main>
  );
}
