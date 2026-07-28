import { Request, Response, NextFunction } from "express";
import logger from "../../config/logger";
import {
  ApplicationError,
  toCategorizedErrorResponse,
  ErrorCategory,
} from "../../utils/error";

/**
 * Interface for the standardized error response with taxonomy fields
 */
interface StandardErrorResponse {
  success: boolean;
  status: number;
  error: {
    message: string;
    code: string;
    category: string;
    recoverable: boolean;
    details?: unknown;
    stack?: string;
  };
}

/**
 * Derive an ErrorCategory from a raw DB error code.
 */
function dbCodeToCategory(dbCode: string): ErrorCategory {
  switch (dbCode) {
    case "23502":
    case "23503":
    case "22001":
    case "22007":
    case "22P02":
    case "23514":
      return "VALIDATION";
    case "23505":
      return "POLICY";
    default:
      return "UNKNOWN";
  }
}

/**
 * Centralized error handling middleware
 */
export async function ErrorHandler(
  err: unknown,
  req: Request,
  res: Response,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  next: NextFunction
) {
  const error = err as {
    message?: string;
    stack?: string;
    code?: string;
    column?: string;
  };
  let statusCode = 500;
  let message = "Internal server error";
  let errorCode = "INTERNAL_SERVER_ERROR";
  let category: ErrorCategory = "UNKNOWN";
  let recoverable = false;
  const details: unknown = undefined;

  if (error instanceof ApplicationError) {
    const categorized = toCategorizedErrorResponse(error);
    statusCode = categorized.statusCode;
    message = categorized.message;
    errorCode = categorized.code;
    category = categorized.category as ErrorCategory;
    recoverable = statusCode >= 500 || statusCode === 429;
  } else if (error.code) {
    category = dbCodeToCategory(error.code);
    recoverable = false;

    switch (error.code) {
      case "23502":
        message = error.column
          ? `Field '${error.column}' cannot be empty.`
          : "A required field is missing.";
        statusCode = 400;
        errorCode = "MISSING_FIELD";
        break;
      case "23505":
        message = "Duplicate entry. This record already exists.";
        statusCode = 409;
        errorCode = "DUPLICATE_ENTRY";
        break;
      case "23503":
        message = "Invalid reference. The related record does not exist.";
        statusCode = 400;
        errorCode = "INVALID_REFERENCE";
        break;
      case "22001":
        message = "Data is too long for the specified field.";
        statusCode = 400;
        errorCode = "VALUE_TOO_LONG";
        break;
      case "22007":
        message = "Invalid date/time format.";
        statusCode = 400;
        errorCode = "INVALID_DATE_FORMAT";
        break;
      case "22P02":
        message = "Invalid input format.";
        statusCode = 400;
        errorCode = "INVALID_INPUT_FORMAT";
        break;
      case "23514":
        message = "Field value does not meet required constraints.";
        statusCode = 400;
        errorCode = "CONSTRAINT_VIOLATION";
        break;
      default:
        if (error.message) message = error.message;
        break;
    }
  } else if (error.message) {
    message = error.message;
  }

  logger.error("Request error", {
    message: error.message || "No message provided",
    statusCode,
    errorCode,
    category,
    method: req.method,
    url: req.originalUrl,
    stack: error.stack,
  });

  const response: StandardErrorResponse = {
    success: false,
    status: statusCode,
    error: {
      message,
      code: errorCode,
      category,
      recoverable,
      details,
      stack: process.env.NODE_ENV === "development" ? error.stack : undefined,
    },
  };

  return res.status(statusCode).json(response);
}
