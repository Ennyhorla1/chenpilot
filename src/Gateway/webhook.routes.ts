import { Router, Request, Response } from "express";
import * as crypto from "crypto";
import { stellarWebhookService } from "./webhook.service";
import { platformWebhookService } from "./platformWebhook.service";
import logger from "../config/logger";

const router = Router();

function verifyWebhookSignature(
  req: Request,
  res: Response,
  next: () => void
): void {
  const secret = process.env.WEBHOOK_SECRET;
  if (!secret) {
    logger.warn("WEBHOOK_SECRET not configured — skipping webhook signature verification");
    next();
    return;
  }

  const signature = req.headers["x-webhook-signature"] as string | undefined;
  if (!signature) {
    res.status(401).json({ success: false, message: "Missing webhook signature" });
    return;
  }

  const rawBody = JSON.stringify(req.body);
  const expected = "sha256=" + crypto.createHmac("sha256", secret).update(rawBody).digest("hex");

  const sigBuffer = Buffer.from(signature);
  const expectedBuffer = Buffer.from(expected);

  if (
    sigBuffer.length !== expectedBuffer.length ||
    !crypto.timingSafeEqual(sigBuffer, expectedBuffer)
  ) {
    logger.warn("Webhook signature mismatch", { receivedSignature: signature });
    res.status(401).json({ success: false, message: "Invalid webhook signature" });
    return;
  }

  next();
}

// Stellar funding webhook
router.post(
  "/stellar/funding",
  verifyWebhookSignature,
  async (req: Request, res: Response) => {
    try {
      const result = await stellarWebhookService.processFundingWebhook(req);
      if (result.success) {
        return res.status(200).json({
          success: true,
          message: result.message,
          userId: result.userId,
          deploymentTriggered: result.deploymentTriggered,
        });
      }
      return res.status(400).json({
        success: false,
        message: result.message,
      });
    } catch (error) {
      console.error("Webhook processing error:", error);
      return res.status(500).json({
        success: false,
        message: "Internal server error",
      });
    }
  }
);

// Telegram webhook
router.post(
  "/telegram",
  verifyWebhookSignature,
  async (req: Request, res: Response) => {
    try {
      const result = await platformWebhookService.processTelegramWebhook(req);
      if (result.isDuplicate) {
        return res.status(200).json({ success: true, message: result.message });
      }
      if (result.success) {
        return res.status(200).json({ success: true, message: result.message, data: result.data });
      }
      return res.status(400).json({ success: false, message: result.message });
    } catch (error) {
      console.error("Telegram webhook processing error:", error);
      return res.status(500).json({
        success: false,
        message: "Internal server error",
      });
    }
  }
);

// Discord webhook
router.post(
  "/discord",
  verifyWebhookSignature,
  async (req: Request, res: Response) => {
    try {
      const result = await platformWebhookService.processDiscordWebhook(req);
      if (
        result.data &&
        typeof result.data === "object" &&
        "type" in result.data &&
        result.data.type === 1
      ) {
        return res.status(200).json({ type: 1 });
      }

      if (result.isDuplicate) {
        return res.status(200).json({ success: true, message: result.message });
      }

      if (result.success) {
        return res.status(200).json({ success: true, message: result.message, data: result.data });
      }

      return res.status(400).json({ success: false, message: result.message });
    } catch (error) {
      console.error("Discord webhook processing error:", error);
      return res.status(500).json({
        success: false,
        message: "Internal server error",
      });
    }
  }
);

export default router;
