Redact KYC PII fields in structured logs
Repo Avatar
gear5labs/chenpilot
The README states the logging system automatically redacts "Passwords, tokens, and private keys." It doesn't mention PII fields that would be present in KYC flows (full name, date of birth, document numbers, addresses). If the redaction allowlist/denylist is keyword-based, KYC-specific fields may not be covered.

Proposed Work

Audit src/config/logger (or equivalent) redaction rules against all fields passed through src/services/kyc
Extend the redaction list to cover KYC PII fields explicitly
Acceptance Criteria

 KYC PII fields confirmed redacted in a test that logs a sample KYC payload and asserts no PII appears in output
 src/config/LOGGING.md updated with the full redaction field list