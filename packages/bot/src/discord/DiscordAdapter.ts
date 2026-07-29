/**
 * Modular Discord Adapter
 * Main Discord adapter using modular components
 */

import { Client, GatewayIntentBits } from 'discord.js';
import { ButtonHandler } from './modules/interaction/ButtonHandler.js';
import { ThreadSafety } from './modules/thread/ThreadSafety.js';
import { RoleGate } from './modules/role/RoleGate.js';
import { SafeBackendClient } from '../commands/services/BackendClient.js';

export interface DiscordAdapterConfig {
  token: string;
  backendUrl: string;
  adminRoleIds: string[];
  intents?: GatewayIntentBits[];
}

export class DiscordAdapter {
  private client: Client;
  private buttonHandler: ButtonHandler;
  private threadSafety: ThreadSafety;
  private roleGate: RoleGate;
  private backendClient: SafeBackendClient;
  private config: DiscordAdapterConfig;

  constructor(config: DiscordAdapterConfig) {
    this.config = config;

    // Initialize Discord client
    this.client = new Client({
      intents: config.intents || [
        GatewayIntentBits.Guilds,
        GatewayIntentBits.GuildMessages,
        GatewayIntentBits.MessageContent,
        GatewayIntentBits.GuildMessageReactions,
      ],
    });

    // Initialize modular components
    this.buttonHandler = new ButtonHandler();
    this.threadSafety = new ThreadSafety();
    this.roleGate = new RoleGate();
    this.backendClient = new SafeBackendClient(config.backendUrl);

    // Configure role gate
    this.roleGate.setAdminRoleIds(config.adminRoleIds);

    // Setup event handlers
    this.setupEventHandlers();
  }

  /**
   * Start the Discord adapter
   */
  async start(): Promise<void> {
    await this.client.login(this.config.token);
  }

  /**
   * Stop the Discord adapter
   */
  async stop(): Promise<void> {
    await this.client.destroy();
    this.threadSafety.clearAllHistory();
    this.roleGate.clearCache();
  }

  /**
   * Setup Discord event handlers
   */
  private setupEventHandlers(): void {
    this.client.on('ready', () => {
      // TODO: Log ready state
    });

    this.client.on('interactionCreate', async (interaction) => {
      await this.handleInteraction(interaction);
    });

    this.client.on('messageCreate', async (message) => {
      await this.handleMessage(message);
    });
  }

  /**
   * Handle Discord interactions
   */
  private async handleInteraction(interaction: any): Promise<void> {
    try {
      if (interaction.isButton()) {
        await this.handleButtonInteraction(interaction);
      } else if (interaction.isModalSubmit()) {
        await this.handleModalInteraction(interaction);
      } else if (interaction.isStringSelectMenu()) {
        await this.handleSelectInteraction(interaction);
      } else if (interaction.isChatInputCommand()) {
        await this.handleCommandInteraction(interaction);
      }
    } catch (error) {
      // TODO: Log error
    }
  }

  /**
   * Handle button interaction
   */
  private async handleButtonInteraction(interaction: any): Promise<void> {
    const discordInteraction = {
      type: 'button' as const,
      id: interaction.id,
      userId: interaction.user.id,
      guildId: interaction.guildId,
      channelId: interaction.channelId,
      data: {
        customId: interaction.customId,
      },
      metadata: {
        timestamp: Date.now(),
        threadId: interaction.channel?.isThread() ? interaction.channelId : undefined,
        parentChannelId: interaction.channel?.parentId,
      },
    };

    const result = await this.buttonHandler.handleButton(discordInteraction);

    if (result.success) {
      await interaction.reply({
        content: result.response?.content || 'Success',
        ephemeral: result.response?.ephemeral,
      });
    } else {
      await interaction.reply({
        content: result.error?.userMessage || result.error?.message || 'An error occurred',
        ephemeral: true,
      });
    }
  }

  /**
   * Handle modal interaction
   */
  private async handleModalInteraction(interaction: any): Promise<void> {
    // TODO: Implement modal handling
  }

  /**
   * Handle select interaction
   */
  private async handleSelectInteraction(interaction: any): Promise<void> {
    // TODO: Implement select handling
  }

  /**
   * Handle command interaction
   */
  private async handleCommandInteraction(interaction: any): Promise<void> {
    const commandName = interaction.commandName;

    // Check role gate
    if (interaction.guildId) {
      const canExecute = await this.roleGate.checkGate(
        commandName,
        interaction.user.id,
        interaction.guildId
      );

      if (!canExecute) {
        await interaction.reply({
          content: 'You do not have permission to use this command',
          ephemeral: true,
        });
        return;
      }
    }

    // TODO: Execute command via backend
    try {
      const result = await this.backendClient.executeCommand(
        commandName,
        interaction.options.data,
        interaction.user.id
      );

      await interaction.reply({
        content: JSON.stringify(result),
        ephemeral: false,
      });
    } catch (error) {
      await interaction.reply({
        content: 'Failed to execute command',
        ephemeral: true,
      });
    }
  }

  /**
   * Handle message
   */
  private async handleMessage(message: any): Promise<void> {
    // Ignore bot messages
    if (message.author.bot) return;

    // TODO: Handle legacy commands
    // TODO: Handle scam detection
  }

  /**
   * Get button handler for registration
   */
  getButtonHandler(): ButtonHandler {
    return this.buttonHandler;
  }

  /**
   * Get thread safety module
   */
  getThreadSafety(): ThreadSafety {
    return this.threadSafety;
  }

  /**
   * Get role gate module
   */
  getRoleGate(): RoleGate {
    return this.roleGate;
  }

  /**
   * Get backend client
   */
  getBackendClient(): SafeBackendClient {
    return this.backendClient;
  }

  /**
   * Get Discord client
   */
  getClient(): Client {
    return this.client;
  }
}
