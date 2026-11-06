# Cloudflare Workers deployment (Next.js 15 + OpenNext)

This is a Workers deployment, not Cloudflare Pages or Vercel. The deployment uses
Cloudflare's documented OpenNext adapter for the existing Next.js 15 app.

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

The preview command builds the OpenNext worker and runs it locally in the Workers
runtime. This is the required runtime smoke test before deploying.

## Temporary Workers URL

Authenticate to Cloudflare, then deploy:

```bash
npx wrangler whoami
npm run cloudflare:deploy
```

With `workers_dev: true`, Wrangler deploys to a temporary
`https://edgenest-frontend.<your-workers-subdomain>.workers.dev` URL. The actual
subdomain is account-specific; do not assume or hardcode it.

## Cloudflare Workers Builds (Git integration)

- Repository: `apostleoffinance/event-driven-trading-rs`
- Root directory: `frontend`
- Build command: `npm run cloudflare:build`
- Deploy command: `npx wrangler deploy`
- Node version: 20.19+ / current supported Node LTS
- Ensure all required `NEXT_PUBLIC_*` build variables are configured in Workers
  Builds. These values are embedded into the client bundle at build time.
- Do not store secrets in `NEXT_PUBLIC_*` variables.

## Production domain (later)

After purchasing and adding `edgenest.com` to Cloudflare:
- attach `app.edgenest.com` as a custom domain to this Worker;
- configure DNS through Cloudflare (avoid manually proxying a conflicting Worker route);
- add WAF rules and rate limits at the edge;
- verify TLS, redirects, and API origins before public launch.

## Security and operational notes

- This frontend currently uses mock repositories and mock event data (see README).
  This Worker deployment does not make the trading backend live.
- Do not expose broker credentials, MT5 secrets, database URLs, or auth secrets to
  browser code or `NEXT_PUBLIC_*` variables.
- WAF/rate limiting must be configured on the relevant Cloudflare zone/hostname
  once the domain exists. Worker-level throttling can be added for specific
  endpoints, but edge rules require Cloudflare account/zone access.
- Deployment requires Cloudflare authentication. The GitHub repo connection alone
  does not authorize this workspace to deploy into the user's Cloudflare account.
