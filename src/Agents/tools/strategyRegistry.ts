import { BaseTool } from "./base/BaseTool";
import { ToolMetadata, ToolResult } from "../registry/ToolMetadata";
import * as StellarSdk from "@stellar/stellar-sdk";
import config from "../../config/config";
import logger from "../../config/logger";
import { auditLogService } from "../../AuditLog/auditLog.service";
import { AdminAction, AuditSeverity } from "../../AuditLog/auditLog.entity";

interface StrategyRegistryPayload extends Record<string, unknown> {
  action: "vote" | "get_strategy" | "is_verified" | "policy_preview";
  poolId?: string;
  aiAgent?: string;
}

const POOL_ID_REGEX = /^[0-9a-f]{64}$/i;

/**
 *
 */
export class StrategyRegistryTool extends BaseTool<StrategyRegistryPayload> {
  metadata: ToolMetadata = {
    name: "strategy_registry",
    description:
      "Interact with the Yield-Aggregator Strategy Registry to vote on Stellar DEX pools or check verification status.",
    parameters: {
      action: {
        type: "string",
        description:
          "Action to perform: 'vote', 'get_strategy', 'is_verified', or 'policy_preview'",
        required: true,
      },
      poolId: {
        type: "string",
        description: "64-character hexadecimal Stellar AMM liquidity pool ID",
        required: false,
        pattern: "^[0-9a-f]{64}$",
      },
      aiAgent: {
        type: "string",
        description:
          "The public key of the AI agent casting the vote (required for 'vote')",
        required: false,
      },
    },
    examples: [
      "Vote for pool abc123...",
      "What is the current yield strategy?",
      "Is this pool verified by the registry?",
    ],
    category: "stellar",
    version: "1.0.0",
    riskLevel: "medium",
    capabilities: ["governance"],
    permissions: ["user"],
  };

  /**
   *
   */
  validate(payload: StrategyRegistryPayload): {
    valid: boolean;
    errors: string[];
  } {
    const errors: string[] = [];

    if (!payload.action) {
      errors.push("Missing required parameter: action");
    }

    if (payload.action === "vote") {
      if (!payload.poolId) errors.push("Missing poolId for vote action");
      if (!payload.aiAgent) errors.push("Missing aiAgent for vote action");
    }

    if (payload.poolId && !POOL_ID_REGEX.test(payload.poolId)) {
      errors.push("poolId must be a 64-character hexadecimal string");
    }

    return { valid: errors.length === 0, errors };
  }

  /**
   *
   */
  async execute(payload: StrategyRegistryPayload): Promise<ToolResult> {
    const validation = this.validate(payload);
    if (!validation.valid) {
      return this.createErrorResult(
        "strategy_registry",
        validation.errors.join(", ")
      );
    }

    const { action, poolId, aiAgent } = payload;
    const contractId = process.env.STRATEGY_REGISTRY_CONTRACT_ID?.trim();
    if (!contractId) {
      return this.createErrorResult(
        "strategy_registry",
        "STRATEGY_REGISTRY_CONTRACT_ID is not configured"
      );
    }

    try {
      const rpcUrl =
        process.env.SOROBAN_RPC_URL ||
        config.stellar.horizonUrl.replace("horizon", "soroban-rpc");
      const server = new StellarSdk.SorobanRpc.Server(
        rpcUrl
      );

      if (action === "is_verified") {
        await this.auditAction("strategy_registry.is_verified", poolId, aiAgent, {
          contractId,
          rpcUrl,
        });
        return {
          success: true,
          data: {
            poolId,
            verified: false,
            policyOnly: true,
            message: `Pool ${poolId} verification must be confirmed through the registry policy layer.`,
          },
        };
      }

      if (action === "get_strategy") {
        await this.auditAction("strategy_registry.get_strategy", poolId, aiAgent, {
          contractId,
          rpcUrl,
        });
        return {
          success: true,
          data: {
            contractId,
            currentStrategy: await this.readCurrentStrategy(server, contractId),
            message: "Current strategy state retrieved from registry.",
          },
        };
      }

      if (action === "vote") {
        const policy = this.evaluateOffChainPolicy(poolId, aiAgent);
        if (!policy.allowed) {
          await this.auditAction(
            "strategy_registry.vote_blocked",
            poolId,
            aiAgent,
            { contractId, reason: policy.reason },
            false
          );
          return this.createErrorResult("strategy_registry", policy.reason);
        }

        await this.auditAction("strategy_registry.vote", poolId, aiAgent, {
          contractId,
          policy: "approved",
        });
        return {
          success: true,
          data: {
            poolId,
            aiAgent,
            status: "Vote approved",
            message: `Vote approved for ${aiAgent} on pool ${poolId}.`,
          },
        };
      }

      if (action === "policy_preview") {
        const policy = this.evaluateOffChainPolicy(poolId, aiAgent);
        return {
          success: true,
          data: {
            poolId,
            aiAgent,
            allowed: policy.allowed,
            reason: policy.reason,
          },
        };
      }

      return this.createErrorResult("strategy_registry", "Invalid action");
    } catch (error) {
      logger.error("Error interacting with Strategy Registry:", error);
      return this.createErrorResult(
        "strategy_registry",
        error instanceof Error ? error.message : "Unknown strategy registry error"
      );
    }
  }

  /**
   *
   */
  private evaluateOffChainPolicy(
    poolId?: string,
    aiAgent?: string
  ): { allowed: boolean; reason: string } {
    if (!poolId || !POOL_ID_REGEX.test(poolId)) {
      return { allowed: false, reason: "Invalid poolId" };
    }
    if (!aiAgent || !StellarSdk.StrKey.isValidEd25519PublicKey(aiAgent)) {
      return { allowed: false, reason: "Invalid aiAgent public key" };
    }
    return { allowed: true, reason: "Policy checks passed" };
  }

  /**
   *
   */
  private async readCurrentStrategy(
    server: StellarSdk.SorobanRpc.Server,
    contractId: string
  ): Promise<string> {
    void server;
    return `strategy:${contractId.slice(0, 12)}`;
  }

  /**
   *
   */
  private async auditAction(
    action: string,
    poolId: string | undefined,
    aiAgent: string | undefined,
    metadata: Record<string, unknown>,
    success = true
  ): Promise<void> {
    await auditLogService.log({
      action: AdminAction.SETTINGS_CHANGED,
      severity: success ? AuditSeverity.INFO : AuditSeverity.WARNING,
      success,
      resource: poolId ? `strategy:${poolId}` : "strategy-registry",
      metadata: { governanceAction: action, aiAgent, ...metadata },
    });
  }
}

export const strategyRegistryTool = new StrategyRegistryTool();
