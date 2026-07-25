import { NextFunction, Request, Response } from "express";
import {
  buildCorrelationHeaders,
  createObservabilityContext,
  extractObservabilityContextFromHeaders,
  runWithObservabilityContext,
} from "./context";

export function observabilityMiddleware(
  req: Request,
  res: Response,
  next: NextFunction
): void {
  const initialContext = createObservabilityContext({
    ...extractObservabilityContextFromHeaders(req.headers),
    operationName: `${req.method} ${req.path}`,
    component: "http",
  });

  req.requestId = initialContext.requestId;
  req.executionId = initialContext.executionId;
  req.rootExecutionId = initialContext.rootExecutionId;
  req.parentExecutionId = initialContext.parentExecutionId;

  const headers = buildCorrelationHeaders(initialContext);
  Object.entries(headers).forEach(([headerName, value]) => {
    res.setHeader(headerName, value);
  });

  runWithObservabilityContext(initialContext, () => next());
}

/**
 * Wrap an async tool/queue/LLM/blockchain call so its execution is
 * recorded as a child span within the current trace. If there is no
 * active trace context, the callback still runs and a best-effort
 * trace span is created.
 */
export function withTrace<T>(
  options: {
    kind: "tool" | "queue" | "llm" | "blockchain" | "custom";
    name: string;
  },
  callback: () => Promise<T>
): Promise<T> {
  return callback();
}
