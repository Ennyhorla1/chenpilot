/**
 * Input Validation Middleware
 * Validates input against schema before command execution
 */

import {
  CommandMiddleware,
  TypedCommandContext,
  TypedCommandResult,
  CommandError,
  ErrorCode,
} from '../types.js';

export function createValidationMiddleware<TInput>(
  schema: any // Zod schema or similar
): CommandMiddleware<TInput> {
  return async (context: TypedCommandContext<TInput>, next) => {
    try {
      // If schema has a parse method (like Zod), use it
      if (schema && typeof schema.parse === 'function') {
        schema.parse(context.input);
      }
      
      return next();
    } catch (error) {
      return {
        success: false,
        error: {
          code: 'VALIDATION_ERROR',
          message: error instanceof Error ? error.message : String(error),
          recoverable: true,
          userMessage: 'Invalid input. Please check your parameters.',
        },
      };
    }
  };
}
