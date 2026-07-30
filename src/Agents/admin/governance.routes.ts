import { Router, Request, Response } from "express";
import { authenticateToken } from "../../Auth/auth.middleware";
import { requireAdmin } from "../../Gateway/middleware/rbac.middleware";
import { promptVersionService } from "../registry/PromptVersionService";
import { promptRolloutService } from "../registry/PromptRolloutService";
import { toolRegistry } from "../registry/ToolRegistry";
import { auditLogService } from "../../AuditLog/auditLog.service";
import { AdminAction, AuditSeverity } from "../../AuditLog/auditLog.entity";
import { requireAdminWorkflow } from "./workflow.middleware";
import { SensitiveActionType } from "./workflow.types";
import AppDataSource from "../../config/Datasource";
import { PromptVersion } from "../registry/PromptVersion.entity";

const router = Router();

router.get("/prompts", authenticateToken, requireAdmin, async (_req, res) => {
  const prompts = await AppDataSource.getRepository(PromptVersion).find({
    order: { createdAt: "DESC" },
  });
  res.json({ success: true, data: prompts });
});

router.get(
  "/prompts/:id/metrics",
  authenticateToken,
  requireAdmin,
  async (req, res) => {
    const metrics = await promptVersionService.getMetrics(req.params.id);
    res.json({ success: true, data: metrics });
  }
);

router.post(
  "/prompts/:id/activate",
  authenticateToken,
  requireAdmin,
  requireAdminWorkflow(SensitiveActionType.ACTIVATE_PROMPT),
  async (req: Request, res: Response) => {
    await promptRolloutService.activateWithPolicy(
      req.params.id,
      req.body.rollbackVersionId
    );
    await auditLogService.log({
      action: AdminAction.SETTINGS_CHANGED,
      severity: AuditSeverity.INFO,
      success: true,
      metadata: {
        domain: "prompt",
        promptId: req.params.id,
        rollbackVersionId: req.body.rollbackVersionId,
      },
    });
    res.json({ success: true });
  }
);

router.get("/tools", authenticateToken, requireAdmin, async (_req, res) => {
  const tools = toolRegistry.getToolMetadata().map((tool) => ({
    name: tool.name,
    version: tool.version,
    category: tool.category,
    riskLevel: tool.riskLevel,
    permissions: tool.permissions || [],
    deprecated: Boolean(tool.deprecated),
  }));
  res.json({ success: true, data: tools });
});

router.put(
  "/tools/:toolName/enable",
  authenticateToken,
  requireAdmin,
  requireAdminWorkflow(SensitiveActionType.ENABLE_TOOL),
  async (req, res) => {
    const tool = toolRegistry.getTool(req.params.toolName);
    if (!tool) {
      return res
        .status(404)
        .json({ success: false, message: "Tool not found" });
    }
    if (req.body.enabled !== false) {
      await auditLogService.log({
        action: AdminAction.SETTINGS_CHANGED,
        severity: AuditSeverity.INFO,
        success: true,
        metadata: {
          domain: "tool",
          toolName: req.params.toolName,
          enabled: true,
        },
      });
    }
    res.json({
      success: true,
      data: {
        toolName: req.params.toolName,
        enabled: req.body.enabled !== false,
      },
    });
  }
);

router.get(
  "/audit/review",
  authenticateToken,
  requireAdmin,
  async (_req, res) => {
    const securityEvents = await auditLogService.getSecurityEvents(24, 100);
    const integrity = await auditLogService.verifyChainIntegrity(1000);
    res.json({ success: true, data: { securityEvents, integrity } });
  }
);

export default router;
