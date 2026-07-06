"use strict";
var __awaiter =
  (this && this.__awaiter) ||
  function (thisArg, _arguments, P, generator) {
    function adopt(value) {
      return value instanceof P
        ? value
        : new P(function (resolve) {
            resolve(value);
          });
    }
    return new (P || (P = Promise))(function (resolve, reject) {
      function fulfilled(value) {
        try {
          step(generator.next(value));
        } catch (e) {
          reject(e);
        }
      }
      function rejected(value) {
        try {
          step(generator["throw"](value));
        } catch (e) {
          reject(e);
        }
      }
      function step(result) {
        result.done
          ? resolve(result.value)
          : adopt(result.value).then(fulfilled, rejected);
      }
      step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
  };
Object.defineProperty(exports, "__esModule", { value: true });
exports.SorobanEventSubscription = void 0;
exports.subscribeToEvents = subscribeToEvents;
exports.parseEvent = parseEvent;
exports.parseVaultEvent = parseVaultEvent;
exports.reconstructVaultState = reconstructVaultState;
const DEFAULT_RPC_URLS = {
  testnet: "https://soroban-testnet.stellar.org",
  mainnet: "https://soroban-mainnet.stellar.org",
};
const DEFAULT_POLLING_INTERVAL_MS = 5000;
class SorobanEventSubscription {
  constructor(config) {
    this.isActive_ = false;
    this.lastLedger_ = null;
    this.pollingHandle_ = null;
    this.eventHandlers = new Set();
    this.errorHandlers = new Set();
    this.processedTransactions = new Set();
    if (!config.contractIds || config.contractIds.length === 0) {
      throw new Error("At least one contractId is required");
    }
    this.config = config;
    this.rpcUrl = config.rpcUrl || DEFAULT_RPC_URLS[config.network];
    if (!this.rpcUrl) {
      throw new Error(`Unknown network: ${config.network}`);
    }
  }
  on(event, handler) {
    if (event === "event") {
      this.eventHandlers.add(handler);
    } else if (event === "error") {
      this.errorHandlers.add(handler);
    }
    return this;
  }
  off(event, handler) {
    if (event === "event") {
      this.eventHandlers.delete(handler);
    } else if (event === "error") {
      this.errorHandlers.delete(handler);
    }
    return this;
  }
  async subscribe() {
    if (this.isActive_) {
      return;
    }
    this.isActive_ = true;
    const interval =
      this.config.pollingIntervalMs ?? DEFAULT_POLLING_INTERVAL_MS;
    await this.poll();
    this.pollingHandle_ = setInterval(() => {
      this.poll().catch((err) => this.emitError(err));
    }, interval);
  }
  async unsubscribe() {
    if (!this.isActive_) {
      return;
    }
    this.isActive_ = false;
    if (this.pollingHandle_) {
      clearInterval(this.pollingHandle_);
      this.pollingHandle_ = null;
    }
    this.eventHandlers.clear();
    this.errorHandlers.clear();
  }
  isActive() {
    return this.isActive_;
  }
  getLastLedger() {
    return this.lastLedger_;
  }
  poll() {
    return __awaiter(this, void 0, void 0, function* () {
      try {
        const events = yield this.fetchRecentEvents();
        for (const event of events) {
          yield this.emitEvent(event);
        }
      } catch (error) {
        this.emitError(
          error instanceof Error ? error : new Error(String(error))
        );
      }
    });
  }
  fetchRecentEvents() {
    return __awaiter(this, void 0, void 0, function* () {
      return [];
    });
  }
  emitEvent(event) {
    return __awaiter(this, void 0, void 0, function* () {
      if (this.processedTransactions.has(event.transactionHash)) {
        return;
      }
      this.processedTransactions.add(event.transactionHash);
      if (this.config.topicFilter && this.config.topicFilter.length > 0) {
        const hasMatchingTopic = event.topics.some((topic) =>
          this.config.topicFilter.some((filter) => topic.includes(filter))
        );
        if (!hasMatchingTopic) {
          return;
        }
      }
      for (const handler of this.eventHandlers) {
        try {
          yield handler(event);
        } catch (err) {
          this.emitError(err instanceof Error ? err : new Error(String(err)));
        }
      }
    });
  }
  emitError(error) {
    for (const handler of this.errorHandlers) {
      try {
        void handler(error);
      } catch (_a) {
        // Ignore errors in error handlers
      }
    }
  }
}
exports.SorobanEventSubscription = SorobanEventSubscription;
function subscribeToEvents(config) {
  return __awaiter(this, void 0, void 0, function* () {
    const subscription = new SorobanEventSubscription(config);
    yield subscription.subscribe();
    return subscription;
  });
}
function parseEvent(raw, contractId, transactionHash, ledger, createdAt) {
  var _a;
  return {
    transactionHash,
    contractId,
    topics: Array.isArray(raw.topic)
      ? raw.topic.map((t) => (typeof t === "string" ? t : JSON.stringify(t)))
      : [],
    data: (_a = raw.value) !== null && _a !== void 0 ? _a : null,
    ledger,
    createdAt,
  };
}
function str(v) {
  return typeof v === "string" ? v : String(v !== null && v !== void 0 ? v : "");
}
function num(v) {
  return typeof v === "number" ? v : Number(v !== null && v !== void 0 ? v : 0);
}
function parseVaultEvent(event) {
  const topic = event.topics[1];
  const contractId = str(event.topics[2]);
  const { ledger, transactionHash: txHash } = event;
  const d = event.data;
  switch (topic) {
    case "init": {
      const data = d;
      return {
        topic,
        contractId,
        version: num(data === null || data === void 0 ? void 0 : data.version),
        ledger,
        actor: str(data === null || data === void 0 ? void 0 : data.actor),
        admin: str(data === null || data === void 0 ? void 0 : data.admin),
        vaultToken: str(data === null || data === void 0 ? void 0 : data.vault_token),
        txHash,
      };
    }
    case "deposit": {
      const data = d;
      return {
        topic,
        contractId,
        user: str(data === null || data === void 0 ? void 0 : data.user),
        amount: str(data === null || data === void 0 ? void 0 : data.amount),
        totalDeposited: str(
          data === null || data === void 0 ? void 0 : data.total_deposited
        ),
        ledger,
        txHash,
      };
    }
    case "w": {
      const data = d;
      return {
        topic,
        contractId,
        user: str(data === null || data === void 0 ? void 0 : data.user),
        amount: str(data === null || data === void 0 ? void 0 : data.amount),
        ledger,
        txHash,
      };
    }
    case "fexit_req": {
      const data = d;
      return {
        topic,
        contractId,
        user: str((d === null || d === void 0 ? void 0 : d.user) ?? contractId),
        amount: str(data === null || data === void 0 ? void 0 : data.amount),
        eligibleAt: num(data === null || data === void 0 ? void 0 : data.eligible_at),
        ledger,
        txHash,
      };
    }
    case "fexit_c": {
      const data = d;
      return {
        topic,
        contractId,
        user: str((d === null || d === void 0 ? void 0 : d.user) ?? contractId),
        amount: str(data === null || data === void 0 ? void 0 : data.amount),
        eligibleAt: num(data === null || data === void 0 ? void 0 : data.eligible_at),
        ledger,
        txHash,
      };
    }
    case "recovery": {
      const data = d;
      return {
        topic,
        contractId,
        user: str((d === null || d === void 0 ? void 0 : d.user) ?? contractId),
        amount: str(data === null || data === void 0 ? void 0 : data.amount),
        reason: (data === null || data === void 0 ? void 0 : data.reason) ?? "AdminIntervention",
        ledger,
        txHash,
      };
    }
    case "upg_prop": {
      const data = d;
      return {
        topic,
        contractId,
        version: num(data === null || data === void 0 ? void 0 : data.version),
        ledger,
        actor: str(data === null || data === void 0 ? void 0 : data.actor),
        admin: str(data === null || data === void 0 ? void 0 : data.actor),
        newWasmHash: str(
          data === null || data === void 0 ? void 0 : data.new_wasm_hash
        ),
        unlockLedger: num(
          data === null || data === void 0 ? void 0 : data.unlock_ledger
        ),
        txHash,
      };
    }
    case "upg_cncl": {
      const data = d;
      return {
        topic,
        contractId,
        version: num(data === null || data === void 0 ? void 0 : data.version),
        ledger,
        actor: str(data === null || data === void 0 ? void 0 : data.actor),
        admin: str(data === null || data === void 0 ? void 0 : data.actor),
        txHash,
      };
    }
    case "upg_done": {
      const data = d;
      return {
        topic,
        contractId,
        version: num(data === null || data === void 0 ? void 0 : data.version),
        ledger,
        actor: str(data === null || data === void 0 ? void 0 : data.actor),
        newWasmHash: str(
          data === null || data === void 0 ? void 0 : data.new_wasm_hash
        ),
        txHash,
      };
    }
    case "adm_xfer": {
      const data = d;
      return {
        topic,
        contractId,
        version: num(data === null || data === void 0 ? void 0 : data.version),
        ledger,
        actor: str(data === null || data === void 0 ? void 0 : data.actor),
        oldAdmin: str(
          data === null || data === void 0 ? void 0 : data.old_admin
        ),
        newAdmin: str(
          data === null || data === void 0 ? void 0 : data.new_admin
        ),
        txHash,
      };
    }
    case "deposit": {
      const data = d;
      return {
        topic,
        contractId,
        version: num(data === null || data === void 0 ? void 0 : data.version),
        ledger,
        actor: str(data === null || data === void 0 ? void 0 : data.actor),
        user: str(data === null || data === void 0 ? void 0 : data.user),
        amount: num(data === null || data === void 0 ? void 0 : data.amount),
        txHash,
      };
    }
    case "force_exit_req": {
      const data = d;
      return {
        topic,
        contractId,
        version: num(data === null || data === void 0 ? void 0 : data.version),
        ledger,
        actor: str(data === null || data === void 0 ? void 0 : data.actor),
        user: str(data === null || data === void 0 ? void 0 : data.user),
        amount: num(data === null || data === void 0 ? void 0 : data.amount),
        eligibleAt: num(
          data === null || data === void 0 ? void 0 : data.eligible_at
        ),
        txHash,
      };
    }
    case "force_exit_done": {
      const data = d;
      return {
        topic,
        contractId,
        version: num(data === null || data === void 0 ? void 0 : data.version),
        ledger,
        actor: str(data === null || data === void 0 ? void 0 : data.actor),
        user: str(data === null || data === void 0 ? void 0 : data.user),
        amount: num(data === null || data === void 0 ? void 0 : data.amount),
        txHash,
      };
    }
    case "backend_status": {
      const data = d;
      return {
        topic,
        contractId,
        version: num(data === null || data === void 0 ? void 0 : data.version),
        ledger,
        actor: str(data === null || data === void 0 ? void 0 : data.actor),
        online: bool(data === null || data === void 0 ? void 0 : data.online),
        txHash,
      };
    }
    default:
      return null;
  }
}
function reconstructVaultState(events) {
  const state = {
    admin: null,
    pendingUpgrade: null,
    currentWasmHash: null,
    deposits: new Map(),
  };
  for (const e of events) {
    switch (e.topic) {
      case "init":
        state.admin = e.admin;
        break;
      case "deposit":
        state.deposits.set(e.user, e.totalDeposited);
        break;
      case "w": {
        const current = state.deposits.get(e.user) ?? "0";
        const currentNum = BigInt(current);
        const withdrawnNum = BigInt(e.amount);
        const remaining = currentNum - withdrawnNum;
        if (remaining <= 0n) {
          state.deposits.delete(e.user);
        } else {
          state.deposits.set(e.user, remaining.toString());
        }
        break;
      }
      case "fexit_req":
        break;
      case "fexit_c":
        state.deposits.delete(e.user);
        break;
      case "recovery":
        // Recovery cancels the pending request - deposit was never removed during force_exit_request
        // No state change needed
        break;
      case "upg_prop":
        state.pendingUpgrade = {
          newWasmHash: e.newWasmHash,
          unlockLedger: e.unlockLedger,
        };
        break;
      case "upg_cncl":
        state.pendingUpgrade = null;
        break;
      case "upg_done":
        state.currentWasmHash = e.newWasmHash;
        state.pendingUpgrade = null;
        break;
      case "adm_xfer":
        state.admin = e.newAdmin;
        break;
      case "deposit":
        state.deposits[e.user] = (state.deposits[e.user] || 0) + e.amount;
        break;
      case "force_exit_req":
        state.forceExits[e.user] = { amount: e.amount, eligibleAt: e.eligibleAt };
        break;
      case "force_exit_done":
        delete state.deposits[e.user];
        state.forceExits[e.user] = null;
        break;
      case "backend_status":
        state.backendOnline = e.online;
        break;
    }
  }
  return state;
}