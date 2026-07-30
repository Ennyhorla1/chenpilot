Add rate limiting specifically to KYC submission endpoints
Repo Avatar
gear5labs/chenpilot
src/services/kyc and KYC_PROVIDER env var indicate KYC integration exists. KYC submission endpoints typically handle PII and third-party API calls with their own rate limits (and cost per call) — these should have tighter, dedicated rate limiting distinct from general API rate limits to prevent abuse (e.g., spamming a paid KYC provider).

Proposed Work

Audit whether KYC endpoints currently share the general rate limiter or have dedicated limits
Add a stricter per-user/per-IP limit scoped to KYC submission routes
Acceptance Criteria

 KYC endpoints have documented, dedicated rate limits
 Test confirming limit enforcement