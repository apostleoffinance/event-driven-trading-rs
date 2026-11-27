# Cloudflare Workers deployment (Next.js 15 + OpenNext)

This is a Workers deployment, not Cloudflare Pages or Vercel. The deployment uses Cloudflare's documented OpenNext adapter for the existing Next.js 15 app.

## Local requirements
- Node.js 20.19+ (use the current supported Node LTS)
- npm
- Cloudflare account with Workers enabled
- Wrangler authenticated via `npx wrangler login`

## Install / prepare
From `frontend/`:
```bash
npm install
cp .dev.vars.example .dev.vars
npm run typecheck
npm run cloudflare:build
npm run cloudflare:preview
```
The preview command builds the OpenNext worker and runs it locally in the Workers runtime. This is the required runtime smoke test before deploying.

**Route verification:** the Next.js route table emitted during the build must include `/login` (from `src/app/login/page.tsx`) and `/market-data`. If either is absent, stop and inspect the local checkout before deploying:
```bash
git status --short
git log -1 --oneline
ls -la src/app/login/page.tsx
```
Do not deploy a Worker whose build route table omits the login page. A successful OpenNext bundle does not prove every intended route is present.

## Frontend API configuration

The browser does not receive API origins or bearer tokens. All browser requests use the same-origin `/api/bff/*` routes; the Worker forwards to the Rust services.

Configure these **server-only Worker Variables** in Cloudflare Worker Settings → Variables:
```env
API_GATEWAY_ORIGIN=https://edgenest-api.onrender.com
IDENTITY_API_ORIGIN=https://edgenest-api.onrender.com
JOURNAL_API_ORIGIN=https://edgenest-api.onrender.com
MARKET_DATA_API_ORIGIN=https://edgenest-api.onrender.com
```

The service-specific origins fall back to `API_GATEWAY_ORIGIN`. Keep values as HTTPS origins without path suffixes. Do not prefix them with `NEXT_PUBLIC_`; they are runtime-only Worker variables, not build variables. For local development, use loopback origins in `.dev.vars` as shown in `.dev.vars.example`.
## Temporary Workers URL
Authenticate to Cloudflare, then deploy:
```bash
npx wrangler whoami
npm run cloudflare:deploy
```
With `workers_dev: true`, Wrangler deploys to a temporary `https://edgenest-frontend.<your-workers-subdomain>.workers.dev` URL. The actual subdomain is account-specific; do not assume or hardcode it.

## Cloudflare Workers Builds (Git integration)
- Repository: `apostleoffinance/event-driven-trading-rs`
- Root directory: `frontend`
- Build command: `npm run cloudflare:build`
- Deploy command: `npx wrangler deploy`
- Node version: 20.19+ / current supported Node LTS
- Configure the four server-only BFF API origins as Worker Variables. Do not expose service origins through `NEXT_PUBLIC_*` build variables.
- Do not store secrets in `NEXT_PUBLIC_*` variables.

## Production domain (later)
After purchasing and adding `edgenest.com` to Cloudflare:
- attach `app.edgenest.com` as a custom domain to this Worker;
- configure DNS through Cloudflare (avoid manually proxying a conflicting Worker route);
- add WAF rules and rate limits at the edge;
- verify TLS, redirects, and API origins before public launch.

## Security and operational notes
- The frontend uses API-backed data only; it does not seed or fabricate trading accounts, positions, orders, or events. A successful Worker deployment does not make broker execution or backend services live.
- Do not expose broker credentials, MT5 secrets, database URLs, or auth secrets to browser code or `NEXT_PUBLIC_*` variables.
- WAF/rate limiting must be configured on the relevant Cloudflare zone/hostname once the domain exists. Worker-level throttling can be added for specific endpoints, but edge rules require Cloudflare account/zone access.
- Deployment requires Cloudflare authentication. The GitHub repo connection alone does not authorize this workspace to deploy into the user's Cloudflare account.
