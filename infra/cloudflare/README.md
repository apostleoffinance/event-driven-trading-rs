# EdgeNest Cloudflare API protections

The `identity-rate-limits.tf` file defines a Cloudflare Zone Ruleset in the `http_ratelimit` phase. It assumes the Cloudflare Terraform provider is configured by the root infrastructure stack and that `var.cloudflare_zone_id` is available.

Add the following input in the root module (do not commit zone IDs or API tokens):

```hcl
variable "cloudflare_zone_id" {
  type        = string
  description = "Cloudflare zone ID for the EdgeNest API domain"
}
```

Deploy through the existing infrastructure pipeline. Do not manually paste API credentials into Terraform source.

## Defaults

| Route family | Limit | Block duration |
|---|---:|---:|
| Login + MFA challenge | 10 / IP / minute | 10 min |
| Registration + email verify/resend | 5 / IP / minute | 15 min |
| Password reset request/confirm | 3 / IP / minute | 15 min |
| Refresh | 30 / IP / minute | 5 min |

These rules are defense-in-depth. Also enable Cloudflare Bot Fight Mode or equivalent bot protections where available, and keep the API behind Cloudflare (restrict origin ingress to Cloudflare IP ranges). Monitor false positives before tightening thresholds. For deployments behind reverse proxies, application rate-limit keys must never trust arbitrary client-supplied `X-Forwarded-For` headers.
