Document required PostgreSQL and Redis versions in README
Repo Avatar
gear5labs/chenpilot
The README lists Node.js 18+ and "a PostgreSQL database" as prerequisites but gives no minimum PostgreSQL version, and doesn't mention Redis at all — despite Redis being a hard dependency for the distributed locking system (src/services/lock) described later in the same README.

Expected Behavior
Prerequisites section should list:

Node.js version (confirm exact minimum)
PostgreSQL minimum version
Redis minimum version (required for LockService)
pnpm version
Acceptance Criteria

 README Prerequisites section updated with all four dependencies and version constraints
 Note added that Redis is required for trade-locking, not optional