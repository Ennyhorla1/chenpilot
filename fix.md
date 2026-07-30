Add good first issue around type-safety: replace remaining any usage in src/
Repo Avatar
gear5labs/chenpilot
CONTRIBUTING.md explicitly states "Use strict typing. Avoid any unless absolutely necessary," which implies some any usage currently exists in the codebase that predates or violates this guideline.

Proposed Work

Run grep -rn ": any" src packages --include="*.ts" and audit each occurrence
Replace with proper types or unknown + type guards where the type is genuinely dynamic
Add an ESLint rule (@typescript-eslint/no-explicit-any) as a warning, escalating to error once cleanup is complete
Acceptance Criteria

 Audit list of all current any usages published in the issue/PR
 At least the highest-risk instances (e.g., in Security, Agents/risk, contracts-adjacent services) resolved
 Lint rule added to prevent regression