.env.local should not be committed to the repository
Repo Avatar
gear5labs/chenpilot
The repository ships both .env.example and .env.local at the root. Committing .env.local risks leaking real credentials (DB, JWT secrets, API keys) if it was ever populated outside a throwaway dev environment, and sets a bad precedent for contributors who may assume it's safe to commit their own local env files.

Steps to Reproduce

Clone the repo
Observe .env.local present in git history alongside .env.example
Expected Behavior
Only .env.example (with placeholder values) should be tracked. .env.local should be gitignored.

Proposed Fix

Add .env.local to .gitignore
Run git rm --cached .env.local
Rotate any secrets that may have been exposed in git history
Add a pre-commit check (or CI secret scanner) to catch future .env* commits
Acceptance Criteria

 .env.local removed from tracked files
 .gitignore updated
 Confirmation that no real secrets were exposed, or secrets rotated if they were