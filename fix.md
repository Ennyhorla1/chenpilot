Add JSDoc/TSDoc coverage requirement for src/Agents/tools/*.ts
Repo Avatar
gear5labs/chenpilot
package.json shows src/Agents/tools/*.ts triggers npm run gen:docs (scripts/generateToolDocs.mjs) on commit via lint-staged, implying tool docs are auto-generated from source comments. If tools lack proper JSDoc annotations, the generated docs (docs/TOOLS.md) will be incomplete or empty for those entries.

Proposed Work

Audit src/Agents/tools/*.ts for missing JSDoc on exported classes/methods (e.g. wallet.ts, swap.ts, riskAnalysis.ts, strategyRegistry.ts)
Add an ESLint rule (eslint-plugin-jsdoc or similar) enforcing JSDoc on new tools going forward
Backfill missing docs for existing tools
Acceptance Criteria

 docs/TOOLS.md regenerated with complete descriptions for every tool
 Lint rule added to prevent future undocumented tools merging