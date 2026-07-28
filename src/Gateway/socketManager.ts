import { Server as SocketIOServer, Socket } from "socket.io";
import { Server as HTTPServer } from "http";
import logger from "../config/logger";
import { EventEmitter } from "events";
import { evaluateRealtimeAbusePolicy } from "../Security";

/**
 * Represents a connected client with metadata
 */
interface ConnectedClient {
  userId: string;
  socketId: string;
  connectedAt: Date;
  userAgent?: string;
  ip?: string;
  role: string;
}

/**
 * Real-time update event types
 */
export enum RealtimeEventType {
  TRANSACTION_STATUS_UPDATE = "transaction:status-update",
  TRANSACTION_CREATED = "transaction:created",
  TRANSACTION_CONFIRMED = "transaction:confirmed",
  TRANSACTION_FAILED = "transaction:failed",
  BOT_ALERT = "bot:alert",
  BOT_STATUS_CHANGE = "bot:status-change",
  BOT_ERROR = "bot:error",
  DEPLOYMENT_STATUS = "deployment:status",
  SWAP_STATUS = "swap:status",
  AGENT_EXECUTION_STARTED = "agent:execution-started",
  AGENT_STEP_COMPLETED = "agent:step-completed",
  AGENT_EXECUTION_COMPLETED = "agent:execution-completed",
  AGENT_EXECUTION_FAILED = "agent:execution-failed",
  AGENT_APPROVAL_REQUIRED = "agent:approval-required",
}

/**
 * Transaction status update payload
 */
export interface TransactionStatusUpdate {
  transactionId: string;
  transactionHash: string;
  status: "pending" | "confirmed" | "failed";
  timestamp: Date;
  ledger?: number;
  feeUsed?: number;
  memo?: string;
  userId?: string;
}

/**
 * Agent execution payload
 */
export interface AgentExecutionUpdate {
  executionId: string;
  planId: string;
  userId: string;
  status: string;
  currentStep?: number;
  totalSteps?: number;
  result?: any;
  error?: string;
  timestamp: Date;
}

/**
 * Bot alert payload
 */
export interface BotAlert {
  alertId: string;
  severity: "info" | "warning" | "error" | "critical";
  message: string;
  botId?: string;
  timestamp: Date;
  userId?: string;
  details?: Record<string, unknown>;
}

/**
 * Bot status change payload
 */
export interface BotStatusChange {
  botId: string;
  status: "active" | "inactive" | "error" | "paused";
  message: string;
  timestamp: Date;
  userId?: string;
}

/**
 * Deployment status payload
 */
export interface DeploymentStatus {
  deploymentId: string;
  status: "pending" | "in-progress" | "completed" | "failed";
  progress?: number;
  message: string;
  timestamp: Date;
  userId?: string;
  details?: Record<string, unknown>;
}

/**
 * Socket.io event emitter for managing real-time updates
 */
export class RealtimeEventEmitter extends EventEmitter {
  constructor() {
    super();
  }

  /**
   * Emit a transaction status update
   */
  emitTransactionUpdate(update: TransactionStatusUpdate): void {
    this.emit(RealtimeEventType.TRANSACTION_STATUS_UPDATE, update);
  }

  /**
   * Emit a transaction created event
   */
  emitTransactionCreated(update: TransactionStatusUpdate): void {
    this.emit(RealtimeEventType.TRANSACTION_CREATED, update);
  }

  /**
   * Emit a transaction confirmed event
   */
  emitTransactionConfirmed(update: TransactionStatusUpdate): void {
    this.emit(RealtimeEventType.TRANSACTION_CONFIRMED, update);
  }

  /**
   * Emit a transaction failed event
   */
  emitTransactionFailed(update: TransactionStatusUpdate): void {
    this.emit(RealtimeEventType.TRANSACTION_FAILED, update);
  }

  /**
   * Emit a bot alert
   */
  emitBotAlert(alert: BotAlert): void {
    this.emit(RealtimeEventType.BOT_ALERT, alert);
  }

  /**
   * Emit a bot status change
   */
  emitBotStatusChange(statusChange: BotStatusChange): void {
    this.emit(RealtimeEventType.BOT_STATUS_CHANGE, statusChange);
  }

  /**
   * Emit a bot error
   */
  emitBotError(alert: BotAlert): void {
    this.emit(RealtimeEventType.BOT_ERROR, alert);
  }

  /**
   * Emit deployment status update
   */
  emitDeploymentStatus(status: DeploymentStatus): void {
    this.emit(RealtimeEventType.DEPLOYMENT_STATUS, status);
  }

  /**
   * Emit swap status update
   */
  emitSwapStatus(update: TransactionStatusUpdate): void {
    this.emit(RealtimeEventType.SWAP_STATUS, update);
  }

  /**
   * Emit agent execution update
   */
  emitAgentExecutionUpdate(
    type: RealtimeEventType,
    update: AgentExecutionUpdate
  ): void {
    this.emit(type, update);
  }
}

/**
 * Socket.io Server Manager
 * Handles real-time communication with connected clients
 */
export class SocketManager {
  private io: SocketIOServer;
  private connectedClients: Map<string, ConnectedClient>;
  private eventEmitter: RealtimeEventEmitter;
  private userSockets: Map<string, Set<string>>; // userId -> Set of socketIds
  private jwtService: JwtService;

  constructor(httpServer: HTTPServer) {
    this.io = new SocketIOServer(httpServer, {
      cors: {
        origin: process.env.ALLOWED_ORIGINS || "*",
        credentials: true,
        methods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
        allowedHeaders: ["Content-Type", "Authorization"],
      },
      transports: ["websocket", "polling"],
      pingInterval: 25000,
      pingTimeout: 60000,
      maxHttpBufferSize: 1e6, // 1 MB max message size
    } as any);

    this.connectedClients = new Map();
    this.userSockets = new Map();
    this.eventEmitter = new RealtimeEventEmitter();
    this.jwtService = container.resolve(JwtService);

    this.setupConnectionHandlers();
    this.setupEventListeners();
  }

  /**
   * Setup socket connection and disconnection handlers
   */
  private setupConnectionHandlers(): void {
    this.io.on("connection", async (socket: Socket) => {
      const ip = socket.handshake.headers["x-forwarded-for"] as string || socket.handshake.address;
      const userAgent = socket.handshake.headers["user-agent"];

      logger.info(`Client connected: ${socket.id} (IP: ${ip})`);

      // Listen for authentication immediately
      socket.once("authenticate", async (token: string) => {
        try {
          const payload = this.jwtService.verifyAccessToken(token);

          const client: ConnectedClient = {
            userId: payload.userId,
            socketId: socket.id,
            connectedAt: new Date(),
            userAgent,
            ip,
            role: payload.role,
          };

          this.connectedClients.set(socket.id, client);

          // Track user's sockets
          if (!this.userSockets.has(payload.userId)) {
            this.userSockets.set(payload.userId, new Set());
          }
          this.userSockets.get(payload.userId)!.add(socket.id);

          socket.join(`user:${payload.userId}`);
          socket.emit("authenticated", { success: true, userId: payload.userId });
          
          // Audit log connection
          await auditLogService.log({
            userId: payload.userId,
            action: AuditAction.SENSITIVE_DATA_ACCESS,
            severity: AuditSeverity.INFO,
            ipAddress: ip,
            userAgent,
            resource: "realtime:connection",
            metadata: {
              event: "connected",
              socketId: socket.id
            },
            success: true,
          });

          logger.info(`Client ${socket.id} authenticated as user ${payload.userId}`);

          // Now set up other listeners AFTER authentication
          this.setupAuthenticatedListeners(socket, client);
        } catch (error) {
          logger.warn(`Authentication failed for client ${socket.id}:`, { error: (error as Error).message });
          socket.emit("error", { message: "Authentication failed. Invalid token." });
          socket.disconnect(true);
        }
      });

      // Handle subscription to transaction updates — requires prior authentication
      socket.on("subscribe:transactions", async (transactionId?: string) => {
        if (!client.userId) {
          socket.emit("error", {
            message: "Authentication required before subscribing.",
          });
          logger.warn(
            `Unauthenticated client ${socket.id} attempted to subscribe to transactions`
          );
          return;
        }
        const abuseDecision = await evaluateRealtimeAbusePolicy(
          "subscribe:transactions",
          {
            userId: client.userId,
            sessionId: socket.id,
            ipAddress: socket.handshake.address,
          },
          { transactionId }
        );
        if (!abuseDecision.allowed) {
          socket.emit("error", {
            message: abuseDecision.reason,
            code: abuseDecision.policyId,
            retryAfterMs: abuseDecision.retryAfterMs,
          });
          return;
        }
        if (transactionId) {
          socket.join(`transaction:${transactionId}`);
          logger.info(
            `Client ${socket.id} subscribed to transaction ${transactionId}`
          );
        }
      });

      // Handle subscription to bot updates — requires prior authentication
      socket.on("subscribe:bot-alerts", async (botId?: string) => {
        if (!client.userId) {
          socket.emit("error", {
            message: "Authentication required before subscribing.",
          });
          logger.warn(
            `Unauthenticated client ${socket.id} attempted to subscribe to bot alerts`
          );
          return;
        }
        const abuseDecision = await evaluateRealtimeAbusePolicy(
          "subscribe:bot-alerts",
          {
            userId: client.userId,
            sessionId: socket.id,
            ipAddress: socket.handshake.address,
          },
          { botId }
        );
        if (!abuseDecision.allowed) {
          socket.emit("error", {
            message: abuseDecision.reason,
            code: abuseDecision.policyId,
            retryAfterMs: abuseDecision.retryAfterMs,
          });
          return;
        }
        if (botId) {
          socket.join(`bot:${botId}`);
          logger.info(`Client ${socket.id} subscribed to bot ${botId}`);
        } else {
          socket.join("bot:all");
          logger.info(`Client ${socket.id} subscribed to all bot alerts`);
        }
      });

      // Handle disconnection
      socket.on("disconnect", async (reason) => {
        clearTimeout(authTimeout);
        const disconnectedClient = this.connectedClients.get(socket.id);
        if (disconnectedClient?.userId) {
          const userSockets = this.userSockets.get(disconnectedClient.userId);
          if (userSockets) {
            userSockets.delete(socket.id);
            if (userSockets.size === 0) {
              this.userSockets.delete(disconnectedClient.userId);
            }
          }

          // Audit log disconnection
          await auditLogService.log({
            userId: disconnectedClient.userId,
            action: AuditAction.SENSITIVE_DATA_ACCESS,
            severity: AuditSeverity.INFO,
            ipAddress: disconnectedClient.ip,
            userAgent: disconnectedClient.userAgent,
            resource: "realtime:connection",
            metadata: {
              event: "disconnected",
              socketId: socket.id,
              reason
            },
            success: true,
          });
        }
        this.connectedClients.delete(socket.id);
        logger.info(`Client disconnected: ${socket.id} (Reason: ${reason})`);
      });

      // Handle errors
      socket.on("error", (error: Error) => {
        logger.error(`Socket error for ${socket.id}:`, {
          error: error.message,
        });
      });
    });
  }

  /**
   * Setup event listeners for authenticated clients
   */
  private setupAuthenticatedListeners(socket: Socket, client: ConnectedClient): void {
    // Handle subscription to transaction updates
    socket.on("subscribe:transactions", async (transactionId?: string) => {
      try {
        if (transactionId) {
          // TODO: Verify user owns this transactionId (add a lookup here later)
          socket.join(`transaction:${transactionId}`);
          logger.info(
            `Client ${socket.id} (user: ${client.userId}) subscribed to transaction ${transactionId}`
          );

          // Audit log subscription
          await auditLogService.log({
            userId: client.userId,
            action: AuditAction.SENSITIVE_DATA_ACCESS,
            severity: AuditSeverity.INFO,
            ipAddress: client.ip,
            userAgent: client.userAgent,
            resource: "realtime:subscription",
            metadata: {
              type: "transactions",
              transactionId,
            },
            success: true,
          });
        }
      } catch (error) {
        logger.error(`Failed to subscribe to transactions:`, { error });
        socket.emit("error", { message: "Failed to subscribe." });
      }
    });

    // Handle subscription to bot updates
    socket.on("subscribe:bot-alerts", async (botId?: string) => {
      try {
        if (botId) {
          // TODO: Verify user owns this botId (add a lookup here later)
          socket.join(`bot:${botId}`);
          logger.info(
            `Client ${socket.id} (user: ${client.userId}) subscribed to bot ${botId}`
          );

          await auditLogService.log({
            userId: client.userId,
            action: AuditAction.SENSITIVE_DATA_ACCESS,
            severity: AuditSeverity.INFO,
            ipAddress: client.ip,
            userAgent: client.userAgent,
            resource: "realtime:subscription",
            metadata: {
              type: "bot-alerts",
              botId,
            },
            success: true,
          });
        }
      } catch (error) {
        logger.error(`Failed to subscribe to bot alerts:`, { error });
        socket.emit("error", { message: "Failed to subscribe." });
      }
    });
  }

  /**
   * Setup event listeners from the event emitter
   */
  private setupEventListeners(): void {
    // Transaction updates
    this.eventEmitter.on(
      RealtimeEventType.TRANSACTION_STATUS_UPDATE,
      (update: TransactionStatusUpdate) => {
        this.broadcastTransactionUpdate(update);
      }
    );

    this.eventEmitter.on(
      RealtimeEventType.TRANSACTION_CREATED,
      (update: TransactionStatusUpdate) => {
        this.broadcastTransactionEvent("created", update);
      }
    );

    this.eventEmitter.on(
      RealtimeEventType.TRANSACTION_CONFIRMED,
      (update: TransactionStatusUpdate) => {
        this.broadcastTransactionEvent("confirmed", update);
      }
    );

    this.eventEmitter.on(
      RealtimeEventType.TRANSACTION_FAILED,
      (update: TransactionStatusUpdate) => {
        this.broadcastTransactionEvent("failed", update);
      }
    );

    this.eventEmitter.on(
      RealtimeEventType.SWAP_STATUS,
      (update: TransactionStatusUpdate) => {
        this.broadcastSwapStatus(update);
      }
    );

    // Bot alerts
    this.eventEmitter.on(RealtimeEventType.BOT_ALERT, (alert: BotAlert) => {
      this.broadcastBotAlert(alert);
    });

    this.eventEmitter.on(
      RealtimeEventType.BOT_STATUS_CHANGE,
      (statusChange: BotStatusChange) => {
        this.broadcastBotStatusChange(statusChange);
      }
    );

    this.eventEmitter.on(RealtimeEventType.BOT_ERROR, (alert: BotAlert) => {
      this.broadcastBotError(alert);
    });

    // Deployment status
    this.eventEmitter.on(
      RealtimeEventType.DEPLOYMENT_STATUS,
      (status: DeploymentStatus) => {
        this.broadcastDeploymentStatus(status);
      }
    );

    // Agent execution updates
    [
      RealtimeEventType.AGENT_EXECUTION_STARTED,
      RealtimeEventType.AGENT_STEP_COMPLETED,
      RealtimeEventType.AGENT_EXECUTION_COMPLETED,
    RealtimeEventType.AGENT_EXECUTION_FAILED,
    RealtimeEventType.AGENT_APPROVAL_REQUIRED,
  ].forEach((eventType) => {
    this.eventEmitter.on(eventType, (update: AgentExecutionUpdate) => {
      this.broadcastAgentUpdate(eventType, update);
    });
  });
  }

  /**
   * Broadcast transaction status update
   */
  private broadcastTransactionUpdate(update: TransactionStatusUpdate): void {
    if (update.userId) {
      this.io.to(`user:${update.userId}`).emit("transaction:update", update);
    }
    this.io
      .to(`transaction:${update.transactionId}`)
      .emit("transaction:update", update);
  }

  /**
   * Broadcast transaction event (created, confirmed, failed)
   */
  private broadcastTransactionEvent(
    eventType: "created" | "confirmed" | "failed",
    update: TransactionStatusUpdate
  ): void {
    const eventName = `transaction:${eventType}`;
    if (update.userId) {
      this.io.to(`user:${update.userId}`).emit(eventName, update);
    }
  }

  /**
   * Broadcast swap status update
   */
  private broadcastSwapStatus(update: TransactionStatusUpdate): void {
    if (update.userId) {
      this.io.to(`user:${update.userId}`).emit("swap:status", update);
    }
    this.io
      .to(`transaction:${update.transactionId}`)
      .emit("swap:status", update);
  }

  /**
   * Broadcast bot alert
   */
  private broadcastBotAlert(alert: BotAlert): void {
    if (alert.userId) {
      this.io.to(`user:${alert.userId}`).emit("bot:alert", alert);
    }
    if (alert.botId) {
      this.io.to(`bot:${alert.botId}`).emit("bot:alert", alert);
    }
  }

  /**
   * Broadcast bot status change
   */
  private broadcastBotStatusChange(statusChange: BotStatusChange): void {
    if (statusChange.userId) {
      this.io
        .to(`user:${statusChange.userId}`)
        .emit("bot:status-change", statusChange);
    }
    this.io
      .to(`bot:${statusChange.botId}`)
      .emit("bot:status-change", statusChange);
    this.io.to("bot:all").emit("bot:status-change", statusChange);
  }

  /**
   * Broadcast bot error
   */
  private broadcastBotError(alert: BotAlert): void {
    if (alert.userId) {
      this.io.to(`user:${alert.userId}`).emit("bot:error", alert);
    }
    if (alert.botId) {
      this.io.to(`bot:${alert.botId}`).emit("bot:error", alert);
    }
  }

  /**
   * Broadcast deployment status
   */
  private broadcastDeploymentStatus(status: DeploymentStatus): void {
    if (status.userId) {
      this.io.to(`user:${status.userId}`).emit("deployment:status", status);
    }
  }

  /**
   * Broadcast agent execution update
   */
  private broadcastAgentUpdate(
    eventType: string,
    update: AgentExecutionUpdate
  ): void {
    if (update.userId) {
      this.io.to(`user:${update.userId}`).emit(eventType, update);
    }
    this.io.to(`execution:${update.executionId}`).emit(eventType, update);
  }

  /**
   * Get the event emitter for external use
   */
  public getEventEmitter(): RealtimeEventEmitter {
    return this.eventEmitter;
  }

  /**
   * Get connected clients count
   */
  public getConnectedClientsCount(): number {
    return this.connectedClients.size;
  }

  /**
   * Get connected clients for a specific user
   */
  public getUserClients(userId: string): ConnectedClient[] {
    const socketIds = this.userSockets.get(userId) || new Set();
    return Array.from(socketIds)
      .map((socketId) => this.connectedClients.get(socketId))
      .filter((client) => client !== undefined) as ConnectedClient[];
  }

  /**
   * Get all connected clients
   */
  public getAllConnectedClients(): ConnectedClient[] {
    return Array.from(this.connectedClients.values());
  }

  /**
   * Get Socket.io server instance
   */
  public getIO(): SocketIOServer {
    return this.io;
  }

  /**
   * Close the socket server
   */
  public async close(): Promise<void> {
    return new Promise((resolve) => {
      this.io.close(() => {
        logger.info("Socket.io server closed");
        resolve();
      });
    });
  }
}

// Global instance
let socketManagerInstance: SocketManager | null = null;

/**
 * Initialize Socket Manager (to be called during server startup)
 */
export function initializeSocketManager(httpServer: HTTPServer): SocketManager {
  if (socketManagerInstance) {
    logger.warn("SocketManager already initialized");
    return socketManagerInstance;
  }
  socketManagerInstance = new SocketManager(httpServer);
  logger.info("SocketManager initialized");
  return socketManagerInstance;
}

/**
 * Get the global Socket Manager instance
 */
export function getSocketManager(): SocketManager {
  if (!socketManagerInstance) {
    throw new Error(
      "SocketManager not initialized. Call initializeSocketManager first."
    );
  }
  return socketManagerInstance;
}
