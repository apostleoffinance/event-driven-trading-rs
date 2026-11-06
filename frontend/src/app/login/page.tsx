"use client";

import Link from "next/link";
import { FormEvent, useState } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/features/auth/AuthProvider";

export default function LoginPage() {
  const { login, completeMfaLogin, ready } = useAuth();
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [challenge, setChallenge] = useState("");
  const [code, setCode] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError("");
    setBusy(true);
    try {
      if (challenge) {
        await completeMfaLogin(challenge, code);
        router.replace("/");
      } else {
        const result = await login(email, password);
        if (result.mfaRequired && result.challengeToken) setChallenge(result.challengeToken);
        else router.replace("/");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sign-in failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="auth-card panel">
      <div className="brand-name">EDGENEST</div>
      <h1>{challenge ? "Two-factor verification" : "Sign in"}</h1>
      <p className="muted">{challenge ? "Enter your authenticator code or recovery code." : "Sign in to your EdgeNest account."}</p>
      <form onSubmit={submit} className="auth-form">
        {!challenge ? (
          <>
            <label>Email<input type="email" autoComplete="email" required value={email} onChange={(event) => setEmail(event.target.value)} /></label>
            <label>Password<input type="password" autoComplete="current-password" required value={password} onChange={(event) => setPassword(event.target.value)} /></label>
          </>
        ) : (
          <label>Authenticator / recovery code<input autoComplete="one-time-code" required value={code} onChange={(event) => setCode(event.target.value)} /></label>
        )}
        {error && <p className="auth-error" role="alert">{error}</p>}
        <button className="btn primary" disabled={busy || !ready}>
          {!ready ? "Restoring session…" : busy ? "Please wait…" : challenge ? "Verify" : "Sign in"}
        </button>
      </form>
      <div className="auth-links"><Link href="/forgot-password">Forgot password?</Link><Link href="/register">Create account</Link></div>
      {!challenge && <p className="muted" style={{fontSize:12}}>Need to verify your email? <Link href="/verify-email">Resend verification</Link></p>}
    </main>
  );
}
