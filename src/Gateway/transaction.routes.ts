import { Router, Request, Response } from "express";
import { transactionHistoryService, type TransactionQueryParams, type TransactionType } from "./transaction.service";
import AppDataSource from "../config/Datasource";
import { User } from "../Auth/user.entity";
import * as StellarSdk from "@stellar/stellar-sdk";
import { SponsorshipTransactionBuilder } from "../../packages/sdk/src/sponsorship";
import logger from "../config/logger";
import { auditLogService } from "../AuditLog/auditLog.service";
import { AuditAction, AuditSeverity } from "../AuditLog/auditLog.entity";
import { authenticateToken } from "../Auth/auth.middleware";
import { requireOwnerOrElevated } from "./middleware/rbac.middleware";

const router = Router();

// Get transaction history
router.get(
  "/:userId/transactions",
  authenticateToken,
  requireOwnerOrElevated("userId"),
  async (req: Request, res: Response) => {
    try {
      const { userId } = req.params;
      if (!userId || Array.isArray(userId)) {
        return res.status(400).json({
          success: false,
          message: "Invalid userId parameter",
        });
      }
      const { type, startDate, endDate, limit, cursor } = req.query as Record<
        string,
        string | undefined
      >;
      const validTypes = ["funding", "deployment", "swap", "transfer", "all"];
      if (type && !validTypes.includes(type)) {
        return res.status(400).json({
          success: false,
          message: `Invalid type. Must be one of: ${validTypes.join(", ")}`,
        });
      }
      const parsedLimit = limit ? parseInt(limit, 10) : 20;
      if (isNaN(parsedLimit) || parsedLimit < 1 || parsedLimit > 100) {
        return res.status(400).json({
          success: false,
          message: "Limit must be a number between 1 and 100",
        });
      }
      if (startDate && isNaN(Date.parse(startDate))) {
        return res.status(400).json({
          success: false,
          message:
            "Invalid startDate format. Use ISO 8601 format (e.g., 2024-01-01T00:00:00Z)",
        });
      }
      if (endDate && isNaN(Date.parse(endDate))) {
        return res.status(400).json({
          success: false,
          message:
            "Invalid endDate format. Use ISO 8601 format (e.g., 2024-01-01T00:00:00Z)",
        });
      }
      const queryParams: TransactionQueryParams = {
        type: type as TransactionType,
        startDate,
        endDate,
        limit: parsedLimit,
        cursor,
      };
      const result = await transactionHistoryService.getTransactionHistory(
        userId,
        queryParams
      );
      return res.status(200).json({ success: true, ...result });
    } catch (error) {
      console.error("Transaction history error:", error);
      const message =
        error instanceof Error ? error.message : "Internal server error";
      const statusCode = message.includes("User not found") ? 404 : 500;
      return res.status(statusCode).json({ success: false, message });
    }
  }
);

// Sponsorship
router.post(
  "/:userId/sponsor",
  authenticateToken,
  requireOwnerOrElevated("userId"),
  async (req: Request, res: Response) => {
    try {
      const { userId } = req.params;
      const userRepository = AppDataSource.getRepository(User);
      const user = await userRepository.findOne({ where: { id: userId } });
      if (!user) {
        return res.status(404).json({ success: false, message: "User not found" });
      }
      if (user.isFunded) {
        return res.status(409).json({ success: false, message: "Account already sponsored" });
      }
      const sponsorSecret = process.env.SPONSOR_SECRET_KEY;
      if (!sponsorSecret) {
        return res.status(503).json({
          success: false,
          message: "Sponsorship service not configured",
        });
      }
      const networkPassphrase =
        process.env.STELLAR_NETWORK === "mainnet"
          ? StellarSdk.Networks.PUBLIC
          : StellarSdk.Networks.TESTNET;
      const sponsorKeypair = StellarSdk.Keypair.fromSecret(sponsorSecret);
      const server = new StellarSdk.Horizon.Server(
        process.env.HORIZON_URL || "https://horizon-testnet.stellar.org"
      );
      await server.loadAccount(sponsorKeypair.publicKey());
      const builder = new SponsorshipTransactionBuilder(
        sponsorKeypair,
        networkPassphrase
      );
      builder.addBeginSponsorship({
        sponsor: sponsorKeypair.publicKey(),
        sponsoredAccount: user.address,
      });
      builder.addSponsoredOperation(
        StellarSdk.Operation.createAccount({
          source: sponsorKeypair.publicKey(),
          destination: user.address,
          startingBalance: "0",
        })
      );
      builder.addEndSponsorship();
      const tx = builder.build();
      tx.sign(sponsorKeypair);
      await server.submitTransaction(tx);
      user.isFunded = true;
      user.updatedAt = new Date();
      await userRepository.save(user);
      await auditLogService.log({
        userId,
        action: AuditAction.USER_CREATED,
        severity: AuditSeverity.INFO,
        ipAddress:
          (req.headers["x-forwarded-for"] as string)?.split(",")[0]?.trim() ||
          (req.headers["x-real-ip"] as string) ||
          req.socket.remoteAddress ||
          "unknown",
        userAgent: req.headers["user-agent"],
        metadata: { event: "account_sponsored", address: user.address },
      });
      return res.status(200).json({
        success: true,
        message: "Account sponsored successfully",
        address: user.address,
      });
    } catch (error) {
      logger.error("Sponsorship error", { error, userId: req.params.userId });
      return res.status(500).json({
        success: false,
        message:
          error instanceof Error ? error.message : "Internal server error",
      });
    }
  }
);

export default router;
