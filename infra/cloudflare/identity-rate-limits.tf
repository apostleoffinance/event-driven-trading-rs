resource "cloudflare_ruleset" "edgenest_identity_rate_limits" {
  zone_id     = var.cloudflare_zone_id
  name        = "EdgeNest Identity API rate limits"
  description = "IP-based rate limits for authentication endpoints"
  kind        = "zone"
  phase       = "http_ratelimit"

  rules {
    description = "Login and MFA challenge brute-force protection"
    enabled     = true
    action      = "block"
    expression  = "(http.request.method eq \"POST\" and http.request.uri.path in {\"/api/v1/auth/login\" \"/api/v1/auth/mfa/verify-login\"})"
    ratelimit {
      characteristics     = ["ip.src"]
      period              = 60
      requests_per_period = 10
      mitigation_timeout  = 600
    }
  }

  rules {
    description = "Registration and email verification abuse protection"
    enabled     = true
    action      = "block"
    expression  = "(http.request.method eq \"POST\" and http.request.uri.path in {\"/api/v1/auth/register\" \"/api/v1/auth/resend-verification\" \"/api/v1/auth/verify-email\"})"
    ratelimit {
      characteristics     = ["ip.src"]
      period              = 60
      requests_per_period = 5
      mitigation_timeout  = 900
    }
  }

  rules {
    description = "Password recovery abuse protection"
    enabled     = true
    action      = "block"
    expression  = "(http.request.method eq \"POST\" and http.request.uri.path in {\"/api/v1/auth/password-reset/request\" \"/api/v1/auth/password-reset/confirm\"})"
    ratelimit {
      characteristics     = ["ip.src"]
      period              = 60
      requests_per_period = 3
      mitigation_timeout  = 900
    }
  }

  rules {
    description = "Refresh token endpoint abuse protection"
    enabled     = true
    action      = "block"
    expression  = "(http.request.method eq \"POST\" and http.request.uri.path eq \"/api/v1/auth/refresh\")"
    ratelimit {
      characteristics     = ["ip.src"]
      period              = 60
      requests_per_period = 30
      mitigation_timeout  = 300
    }
  }
}
