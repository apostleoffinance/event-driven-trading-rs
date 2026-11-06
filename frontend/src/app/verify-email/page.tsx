"use client";
import Link from "next/link";
import { useEffect,useState } from "react";
import { useAuth } from "@/features/auth/AuthProvider";
export default function VerifyEmailPage(){
 const {verifyEmail,resendVerification}=useAuth();const [status,setStatus]=useState("Verifying email link…");const [email,setEmail]=useState("");const [error,setError]=useState("");
 useEffect(()=>{const token=new URLSearchParams(window.location.search).get("token");if(!token){setStatus("Verification link is missing.");return;}verifyEmail(token).then(()=>setStatus("Email verified. You can sign in now.")).catch(e=>setStatus(e instanceof Error?e.message:"Verification failed."));},[verifyEmail]);
 async function resend(){setError("");try{await resendVerification(email);setError("If this account needs verification, an email was sent.");}catch(e){setError(e instanceof Error?e.message:"Could not resend verification.");}}
 return <main className="auth-card panel"><div className="brand-name">EDGENEST</div><h1>Email verification</h1><p>{status}</p><label className="auth-label">Resend verification<input type="email" placeholder="you@example.com" value={email} onChange={e=>setEmail(e.target.value)}/></label><button className="btn" onClick={resend}>Resend email</button>{error&&<p>{error}</p>}<Link href="/login">Back to sign in</Link></main>;
}