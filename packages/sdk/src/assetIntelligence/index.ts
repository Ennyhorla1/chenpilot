/**
 * Asset Intelligence Layer - Public API
 * Main entry point for the Asset Intelligence Layer
 */

export { AssetIntelligence } from './core/AssetIntelligence.js';
export { AssetCache } from './core/AssetCache.js';
export { CacheInvalidator } from './core/CacheInvalidator.js';
export * from './core/types.js';

export { TrustScorer } from './trust/TrustScorer.js';
export { TrustSignals } from './trust/TrustSignals.js';
export { TrustRegistry } from './trust/TrustRegistry.js';

export { MemoryCache } from './cache/MemoryCache.js';
export { PersistentCache } from './cache/PersistentCache.js';
export { CacheKey } from './cache/CacheKey.js';
export { CachePolicy, EvictionPolicy } from './cache/CachePolicy.js';

export { AssetValidator } from './compatibility/AssetValidator.js';
export { NetworkCompatibility } from './compatibility/NetworkCompatibility.js';
export { VersionCompatibility } from './compatibility/VersionCompatibility.js';

export { AssetCacheAdapter, MetadataManagerAdapter, createMigrationAdapters, MIGRATION_GUIDE } from './MigrationAdapter.js';
