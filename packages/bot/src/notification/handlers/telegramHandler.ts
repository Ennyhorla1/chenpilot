/**
 * Telegram Notification Handler
 * Platform-specific delivery handler for Telegram notifications
 */

import {
  PlatformDeliveryHandler,
  NotificationMessage,
  DeliveryResult,
  DeliveryPlatform,
  DeliveryStatus,
} from '../core.js';

/**
 * Telegram notification handler
 */
export class TelegramNotificationHandler implements PlatformDeliveryHandler {
  platform = DeliveryPlatform.TELEGRAM;
  private telegramAdapter: any;
  private available: boolean;
  private latency: number;
  private errorCount: number;
  private totalRequests: number;

  constructor(telegramAdapter: any) {
    this.telegramAdapter = telegramAdapter;
    this.available = !!telegramAdapter;
    this.latency = 0;
    this.errorCount = 0;
    this.totalRequests = 0;
  }

  /**
   * Deliver notification to Telegram
   */
  async deliver(message: NotificationMessage): Promise<DeliveryResult> {
    const startTime = Date.now();

    try {
      this.totalRequests++;

      if (!this.telegramAdapter) {
        throw new Error('Telegram adapter not available');
      }

      // Send notification via Telegram adapter
      // This assumes the adapter has a sendNotification method
      if (typeof this.telegramAdapter.sendNotification === 'function') {
        await this.telegramAdapter.sendNotification(message.userId, message.content);
      } else if (typeof this.telegramAdapter.sendMessage === 'function') {
        // Alternative method name
        await this.telegramAdapter.sendMessage(message.userId, message.content);
      } else {
        throw new Error('Telegram adapter does not have sendNotification method');
      }

      this.latency = Date.now() - startTime;

      return {
        success: true,
        platform: DeliveryPlatform.TELEGRAM,
        attempt: 1,
        duration: this.latency,
        timestamp: Date.now(),
      };
    } catch (error) {
      this.errorCount++;
      this.latency = Date.now() - startTime;

      return {
        success: false,
        platform: DeliveryPlatform.TELEGRAM,
        error: error instanceof Error ? error.message : String(error),
        errorCode: 'TELEGRAM_DELIVERY_ERROR',
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
    return this.available && !!this.telegramAdapter;
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
