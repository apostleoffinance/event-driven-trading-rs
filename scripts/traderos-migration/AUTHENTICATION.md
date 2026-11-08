# TraderOS migration authentication

The migration CLI authenticates to Journal with EdgeNest Identity access tokens. Access tokens are short-lived (the Identity service defaults to 15 minutes), so the migration CLI supports automatic refresh-token rotation.

## Private config

Add the Identity API base URL and the refresh token returned from an EdgeNest Identity login to the existing private config. Keep your current `owner_map` entry and object-storage settings:

```json
{
  "edgenest_api_url": "https://<journal-api-host>",
  "edgenest_identity_api_url": "https://<identity-api-host>",
  "owner_map": {
    "<legacy-traderos-user-id>": {
      "jwt": "<access-token>",
      "refresh_token": "<refresh-token>"
    }
  }
}
```

Use the real Identity API host from your deployment. Do not assume the Journal API and Identity API share a hostname. The CLI calls `POST /api/v1/auth/refresh` with `{"refresh_token":"..."}`. Identity returns a new access token and rotated refresh token. The CLI atomically persists both updated values in the private config and retries the original Journal request once.

Refresh tokens are single-use and rotate on refresh. The CLI saves the new refresh token before continuing. The private config is chmod'd to `0600` when it is updated. Do not commit the config or paste tokens into logs/chat.

## First-time setup

1. Sign in to EdgeNest Identity (or use the Identity login endpoint) to obtain the initial access + refresh token pair.
2. Put both values in the private config as shown above.
3. Run `auth-check` before migration:

```bash
python scripts/traderos-migration/migrate.py auth-check \
  --snapshot migration/traderos/snapshot.json \
  --config migration/traderos/config.private.json
```

The auth check refreshes automatically if the access token is rejected and a valid refresh token is configured. A rejected/expired refresh token requires signing in again and replacing both tokens.

## Security

- Never store passwords in the migration config or source control.
- Refresh tokens are bearer credentials; treat them like passwords.
- Do not copy token values into terminal transcripts, tickets, GitHub issues, or chat.
- Token refresh does not extend JWT expiry. It obtains a new short-lived access token from Identity.
