/**
 * Discord Notification Handler
 * Platform-specific delivery handler for Discord notifications
 */

import {
  PlatformDeliveryHandler,
  NotificationMessage,
  DeliveryResult,
  DeliveryPlatform,
  DeliveryStatus,
} from '../core.js';

/**
 * Discord notification handler
 */
export class DiscordNotificationHandler implements PlatformDeliveryHandler {
  platform = DeliveryPlatform.DISCORD;
  private discordAdapter: any;
  private available: boolean;
  private latency: number;
  private errorCount: number;
  private totalRequests: number;

  constructor(discordAdapter: any) {
    this.discordAdapter = discordAdapter;
    this.available = !!discordAdapter;
    this.latency = 0;
    this.errorCount = 0;
    this.totalRequests = 0;
  }

  /**
   * Deliver notification to Discord
   */
  async deliver(message: NotificationMessage): Promise<DeliveryResult> {
    const startTime = Date.now();

    try {
      this.totalRequests++;

      if (!this.discordAdapter) {
        throw new Error('Discord adapter not available');
      }

      // Send notification via Discord adapter
      // This assumes the adapter has a sendNotification method
      if (typeof this.discordAdapter.sendNotification === 'function') {
        await this.discordAdapter.sendNotification(message.userId, message.content, message.embed);
      } else if (typeof this.discordAdapter.sendMessage === 'function') {
        // Alternative method name
        await this.discordAdapter.sendMessage(message.userId, message.content, message.embed);
      } else {
        throw new Error('Discord adapter does not have sendNotification method');
      }

      this.latency = Date.now() - startTime;

      return {
        success: true,
        platform: DeliveryPlatform.DISCORD,
        attempt: 1,
        duration: this.latency,
        timestamp: Date.now(),
      };
    } catch (error) {
      this.errorCount++;
      this.latency = Date.now() - startTime;

      return {
        success: false,
        platform: DeliveryPlatform.DISCORD,
        error: error instanceof Error ? error.message : String(error),
        errorCode: 'DISCORD_DELIVERY_ERROR',
        attempt: 1,
        duration: this.latency,
        timestamp: Date.now(),
      };
    }
  }

  /**
   * Check if handler is available
   */
  isAvailable(): boolean {
    return this.available && !!this.discordAdapter;
  }

  /**
   * Get handler health status
   */
  getHealth(): {
    available: boolean;
    latency?: number;
    errorRate?: number;
  } {
    const errorRate = this.totalRequests > 0 ? this.errorCount / this.totalRequests : 0;

    return {
      available: this.available,
      latency: this.latency || undefined,
      errorRate: errorRate || undefined,
    };
  }

  /**
   * Set adapter availability
   */
  setAvailable(available: boolean): void {
    this.available = available;
  }

  /**
   * Reset error counters
   */
  resetErrors(): void {
    this.errorCount = 0;
    this.totalRequests = 0;
  }
}
