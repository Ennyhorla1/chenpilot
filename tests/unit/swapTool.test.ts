import { describe, it, expect, jest, beforeEach, afterEach } from "@jest/globals";
import { SwapTool } from "../../src/Agents/tools/swap";
import { transactionLifecycleService } from "../../src/transactions/TransactionLifecycle.service";

jest.mock("../../src/services/redis/client", () => ({
  getRedisClient: jest.fn(() => ({
    set: jest.fn(),
    eval: jest.fn(),
    del: jest.fn(),
    exists: jest.fn(),
    ttl: jest.fn(),
    pipeline: jest.fn(),
  })),
  healthCheckRedis: jest.fn(),
}));

describe("SwapTool lock heartbeat", () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  it("extends the trade lock while a swap is still executing", async () => {
    const tool = new SwapTool();
    const lockService = (tool as unknown as { lockService: any }).lockService;

    jest.spyOn(lockService, "acquireLock").mockResolvedValue({
      acquired: true,
      lockKey: "trade:user-1",
      lockValue: "user-1:abc",
    });
    const extendSpy = jest.spyOn(lockService, "extendLock").mockResolvedValue(true);
    jest.spyOn(lockService, "releaseLock").mockResolvedValue(true);

    jest.spyOn(transactionLifecycleService, "create").mockResolvedValue({
      id: "lifecycle-1",
    } as never);
    jest.spyOn(transactionLifecycleService, "fail").mockResolvedValue(undefined as never);

    const result = await tool.execute(
      { from: "XLM", to: "XLM", amount: 10 },
      "user-1"
    );

    expect(result.success).toBe(false);
    expect(extendSpy).toHaveBeenCalledWith("trade:user-1", "user-1", 60000);
  });
});
