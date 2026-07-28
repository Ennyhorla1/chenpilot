/**
 * Capability Discovery Layer — Issue #378
 *
 * Lets clients discover backend capabilities, contract versions, and feature
 * availability dynamically so integrations can adapt safely to platform evolution.
 *
 * Usage:
 * ```typescript
 * import { CapabilityDiscovery } from "@chenpilot/sdk";
 *
 * const discovery = new CapabilityDiscovery({ network: "testnet" });
 * const caps = await discovery.getCapabilities();
 *
 * if (caps.features.sorobanEnabled) {
 *   // safe to use Soroban APIs
 * }
 *
 * const negotiated = await discovery.negotiate({ minProtocol: 21 });
 * if (!negotiated.compatible) throw new Error(negotiated.reason);
 * ```
 */

// ─── Types ────────────────────────────────────────────────────────────────────

/** Versions of backend contracts/components. */
export interface ContractVersions {
  /** Stellar protocol version (e.g. 21). */
  protocol: number;
  /** Core SDK / server API version (semver). */
  api: string;
  /** Soroban contract version, if deployed. */
  soroban?: string;
  /** Multi-hop contract version, if deployed. */
  multiHop?: string;
  /** Agent contract version, if deployed. */
  agent?: string;
}

/** Runtime feature flags. */
export interface FeatureFlags {
  /** Soroban smart-contract execution is enabled. */
  sorobanEnabled: boolean;
  /** Multi-hop cross-chain bridging is available. */
  multiHopEnabled: boolean;
  /** Streaming real-time events via SSE/WebSocket is supported. */
  realtimeEnabled: boolean;
  /** Claimable balance operations are supported. */
  claimableBalancesEnabled: boolean;
  /** Fee-bump transaction wrapping is available. */
  feeBumpingEnabled: boolean;
  /** Sponsored reserve operations are available. */
  sponsorshipEnabled: boolean;
  /** On-chain metadata storage is available. */
  metadataEnabled: boolean;
}

/** Operational limits reported by the backend. */
export interface BackendLimits {
  /** Maximum simultaneous connections per API key. */
  maxConnections: number;
  /** Requests allowed per minute per API key. */
  rateLimitPerMinute: number;
  /** Maximum XDR payload size in bytes. */
  maxXdrBytes: number;
  /** Maximum age of a cached ledger response (ms). */
  maxCacheAgeMs: number;
}

/** Full capability snapshot returned by the discovery layer. */
export interface BackendCapabilities {
  /** Stellar network identifier ("testnet" or "mainnet"). */
  network: string;
  /** Resolved contract and API versions. */
  versions: ContractVersions;
  /** Enabled feature flags. */
  features: FeatureFlags;
  /** Operational limits. */
  limits: BackendLimits;
  /** ISO-8601 timestamp at which this snapshot was fetched. */
  fetchedAt: string;
  /** Horizon endpoint used for this snapshot. */
  horizonUrl: string;
  /** Soroban RPC endpoint used for this snapshot. */
  rpcUrl: string;
}

/** Input for client-side version negotiation. */
export interface NegotiationRequest {
  /** Minimum Stellar protocol version the client requires. */
  minProtocol?: number;
  /** Minimum API version the client requires (semver). */
  minApi?: string;
  /** Features the client requires (all must be enabled). */
  requiredFeatures?: Array<keyof FeatureFlags>;
}

/** Result of version / capability negotiation. */
export interface NegotiationResult {
  /** True when all client requirements are satisfied. */
  compatible: boolean;
  /** Human-readable explanation when not compatible. */
  reason?: string;
  /** The resolved capability snapshot. */
  capabilities: BackendCapabilities;
}

/** Configuration for CapabilityDiscovery. */
export interface CapabilityDiscoveryConfig {
  /** Network identifier ("testnet" | "mainnet"). */
  network: string;
  /** Override Horizon URL. */
  horizonUrl?: string;
  /** Override Soroban RPC URL. */
  rpcUrl?: string;
  /** How long (ms) to cache a capability snapshot before re-fetching.
   *  Default: 60 000 ms (1 minute). */
  cacheTtlMs?: number;
  /** Request timeout in ms. Default: 10 000 ms. */
  timeout?: number;
}

// ─── Constants ────────────────────────────────────────────────────────────────

const HORIZON_URLS: Record<string, string> = {
  testnet: "https://horizon-testnet.stellar.org",
  mainnet: "https://horizon.stellar.org",
};

const RPC_URLS: Record<string, string> = {
  testnet: "https://soroban-testnet.stellar.org",
  mainnet: "https://soroban-mainnet.stellar.org",
};

const CURRENT_API_VERSION = "1.0.0";
const DEFAULT_CACHE_TTL_MS = 60_000;
const DEFAULT_TIMEOUT_MS = 10_000;

// ─── Helpers ─────────────────────────────────────────────────────────────────

function semverMajor(version: string): number {
  return parseInt(version.split(".")[0] ?? "0", 10);
}

function semverSatisfies(required: string, available: string): boolean {
  const req = required.split(".").map(Number);
  const avail = available.split(".").map(Number);
  for (let i = 0; i < req.length; i++) {
    if ((avail[i] ?? 0) > (req[i] ?? 0)) return true;
    if ((avail[i] ?? 0) < (req[i] ?? 0)) return false;
  }
  return true;
}

// ─── CapabilityDiscovery ─────────────────────────────────────────────────────

/**
 * Discovers backend capabilities, contract versions, and feature flags
 * dynamically by querying the Stellar network. Results are cached for
 * `cacheTtlMs` milliseconds to avoid hammering the RPC on every call.
 */
export class CapabilityDiscovery {
  private readonly config: Required<CapabilityDiscoveryConfig>;
  private cache: BackendCapabilities | null = null;
  private cacheTime = 0;

  constructor(config: CapabilityDiscoveryConfig) {
    this.config = {
      network: config.network,
      horizonUrl: config.horizonUrl ?? HORIZON_URLS[config.network] ?? HORIZON_URLS["testnet"],
      rpcUrl: config.rpcUrl ?? RPC_URLS[config.network] ?? RPC_URLS["testnet"],
      cacheTtlMs: config.cacheTtlMs ?? DEFAULT_CACHE_TTL_MS,
      timeout: config.timeout ?? DEFAULT_TIMEOUT_MS,
    };
  }

  /**
   * Fetch (or return cached) backend capabilities.
   *
   * Performs two lightweight network requests:
   *   1. Horizon /  — resolves protocol version, horizon version.
   *   2. Soroban RPC getLatestLedger — confirms Soroban reachability.
   *
   * Falls back gracefully: if Soroban is unreachable, sorobanEnabled = false.
   */
  async getCapabilities(): Promise<BackendCapabilities> {
    if (this.cache && Date.now() - this.cacheTime < this.config.cacheTtlMs) {
      return this.cache;
    }

    const [horizonInfo, rpcInfo] = await Promise.allSettled([
      this._fetchHorizonRoot(),
      this._fetchRpcLedger(),
    ]);

    const protocol = horizonInfo.status === "fulfilled"
      ? (horizonInfo.value.core_supported_protocol_version ?? 0)
      : 0;

    const sorobanEnabled = rpcInfo.status === "fulfilled";

    const caps: BackendCapabilities = {
      network: this.config.network,
      horizonUrl: this.config.horizonUrl,
      rpcUrl: this.config.rpcUrl,
      fetchedAt: new Date().toISOString(),
      versions: {
        protocol,
        api: CURRENT_API_VERSION,
        soroban: sorobanEnabled ? "21.0" : undefined,
      },
      features: {
        sorobanEnabled,
        multiHopEnabled: sorobanEnabled,
        realtimeEnabled: true,
        claimableBalancesEnabled: protocol >= 15,
        feeBumpingEnabled: protocol >= 13,
        sponsorshipEnabled: protocol >= 14,
        metadataEnabled: sorobanEnabled,
      },
      limits: {
        maxConnections: 100,
        rateLimitPerMinute: 600,
        maxXdrBytes: 1_048_576,
        maxCacheAgeMs: this.config.cacheTtlMs,
      },
    };

    this.cache = caps;
    this.cacheTime = Date.now();
    return caps;
  }

  /**
   * Negotiate compatibility between the client requirements and the backend.
   *
   * @example
   * ```typescript
   * const result = await discovery.negotiate({
   *   minProtocol: 21,
   *   requiredFeatures: ["sorobanEnabled", "metadataEnabled"],
   * });
   * if (!result.compatible) throw new Error(result.reason);
   * ```
   */
  async negotiate(req: NegotiationRequest): Promise<NegotiationResult> {
    const caps = await this.getCapabilities();
    const reasons: string[] = [];

    if (req.minProtocol !== undefined && caps.versions.protocol < req.minProtocol) {
      reasons.push(
        `Protocol ${caps.versions.protocol} < required minimum ${req.minProtocol}`
      );
    }

    if (req.minApi !== undefined && !semverSatisfies(req.minApi, caps.versions.api)) {
      reasons.push(
        `API version ${caps.versions.api} does not satisfy minimum ${req.minApi}`
      );
    }

    for (const feature of req.requiredFeatures ?? []) {
      if (!caps.features[feature]) {
        reasons.push(`Required feature "${feature}" is not available on ${caps.network}`);
      }
    }

    return {
      compatible: reasons.length === 0,
      reason: reasons.length > 0 ? reasons.join("; ") : undefined,
      capabilities: caps,
    };
  }

  /** Invalidate the capability cache, forcing a fresh fetch on next call. */
  invalidateCache(): void {
    this.cache = null;
    this.cacheTime = 0;
  }

  // ─── Private helpers ───────────────────────────────────────────────────────

  private async _fetchHorizonRoot(): Promise<Record<string, unknown>> {
    const signal = AbortSignal.timeout(this.config.timeout);
    const res = await fetch(this.config.horizonUrl, { signal });
    if (!res.ok) throw new Error(`Horizon root ${res.status}`);
    return res.json() as Promise<Record<string, unknown>>;
  }

  private async _fetchRpcLedger(): Promise<unknown> {
    const signal = AbortSignal.timeout(this.config.timeout);
    const res = await fetch(this.config.rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "getLatestLedger", params: [] }),
      signal,
    });
    if (!res.ok) throw new Error(`RPC ${res.status}`);
    return res.json();
  }
}

// ─── Convenience factory ─────────────────────────────────────────────────────

/**
 * Create a CapabilityDiscovery instance for the given network.
 *
 * ```typescript
 * const discovery = createCapabilityDiscovery("testnet");
 * const caps = await discovery.getCapabilities();
 * ```
 */
export function createCapabilityDiscovery(
  network: string,
  options?: Partial<CapabilityDiscoveryConfig>
): CapabilityDiscovery {
  return new CapabilityDiscovery({ network, ...options });
}
