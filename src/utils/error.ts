import express, { NextFunction } from "express";

export type ErrorCategory =
  | "TRANSPORT"
  | "VALIDATION"
  | "SIMULATION"
  | "POLICY"
  | "COMPATIBILITY"
  | "EXECUTION"
  | "UNKNOWN";

/**
 * Map an HTTP status code to an ErrorCategory.
 */
export function httpStatusToCategory(status: number): ErrorCategory {
  if (status === 429) return "POLICY";
  if (status === 422 || status === 400) return "VALIDATION";
  if (status === 401 || status === 403) return "POLICY";
  if (status === 404) return "VALIDATION";
  if (status >= 500) return "EXECUTION";
  return "UNKNOWN";
}

export class ApplicationError extends Error {
  statusCode: number;
  message: string;
  errorCategory: ErrorCategory;
  errorCode?: string;

  constructor(message: string, statusCode: number, errorCode?: string) {
    super(message);
    this.statusCode = statusCode;
    this.message = message;
    this.errorCategory = httpStatusToCategory(statusCode);
    this.errorCode = errorCode;
    Error.captureStackTrace(this, this.constructor);
  }
}

export function toCategorizedErrorResponse(err: ApplicationError): {
  message: string;
  code: string;
  category: string;
  statusCode: number;
} {
  return {
    message: err.message,
    code: err.errorCode ?? `${err.constructor.name
      .replace(/([A-Z])/g, "_$1")
      .toUpperCase()
      .replace(/^_/, "")}`,
    category: err.errorCategory,
    statusCode: err.statusCode,
  };
}

export class NotFoundError extends ApplicationError {
  constructor(message: string, statusCode = 404) {
    super(message, statusCode);
  }
}

export class BadError extends ApplicationError {
  constructor(message: string, statusCode = 400) {
    super(message, statusCode);
  }
}

export class InternalServerError extends ApplicationError {
  constructor(message: string, statusCode = 500) {
    super(message, statusCode);
  }
}

export class ConflictError extends ApplicationError {
  constructor(message: string, statusCode = 409) {
    super(message, statusCode);
  }
}

export class RequiredFieldsError extends ApplicationError {
  constructor(message: string, statusCode = 422) {
    super(message, statusCode);
  }
}

export class ValidationError extends ApplicationError {
  constructor(message: string, statusCode = 422) {
    super(message, statusCode);
  }
}

export class InvalidPayloadError extends ApplicationError {
  constructor(message: string, statusCode = 400) {
    super(message, statusCode);
  }
}

export class MissingFieldsError extends ApplicationError {
  constructor(message: string, statusCode = 400) {
    super(message, statusCode);
  }
}

export class ForbiddenError extends ApplicationError {
  constructor(message: string, statusCode = 403) {
    super(message, statusCode);
  }
}
export class UnauthorizedError extends ApplicationError {
  constructor(message: string, statusCode = 401) {
    super(message, statusCode);
  }
}

const RouteErrorHandler =
  (
    fn: (
      req: express.Request,
      res: express.Response,
      next: NextFunction
    ) => Promise<unknown>
  ) =>
  (req: express.Request, res: express.Response, next: NextFunction) =>
    Promise.resolve(fn(req, res, next)).catch((error) => next(error));

export default RouteErrorHandler;
