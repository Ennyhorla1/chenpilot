/**
 * Authentication Middleware
 * Verifies user identity before command execution
 */

import {
  CommandMiddleware,
  TypedCommandContext,
  TypedCommandResult,
  CommandError,
  ErrorCode,
} from '../types.js';

export function createAuthMiddleware(authService: any): CommandMiddleware<any> {
  return async (context: TypedCommandContext<any>, next) => {
    try {
      const isAuthenticated = await authService.verifyUser(context.userId);
      
      if (!isAuthenticated) {
        return {
          success: false,
          error: {
            code: 'AUTHENTICATION_FAILED',
            message: 'User not authenticated',
            recoverable: false,
            userMessage: 'Please authenticate first',
          },
        };
      }

      return next();
    } catch (error) {
      return {
        success: false,
        error: {
          code: 'AUTHENTICATION_FAILED',
          message: error instanceof Error ? error.message : String(error),
          recoverable: false,
        },
      };
    }
  };
}
