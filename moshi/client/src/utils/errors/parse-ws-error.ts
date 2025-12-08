import {
    CloseCode,
    type ConnectionError,
    type ErrorCategory,
} from "../../types/errors";

/**
 * Base retry delay in milliseconds for exponential backoff.
 */
const BASE_RETRY_DELAY_MS = 1000;

/**
 * Maximum retry delay in milliseconds.
 */
const MAX_RETRY_DELAY_MS = 30000;

/**
 * Maps a WebSocket close code to an error category.
 */
function getErrorCategory(code: number): ErrorCategory {
  switch (code) {
    case CloseCode.Normal:
    case CloseCode.GoingAway:
      return "connection";

    case CloseCode.AuthenticationFailed:
      return "authentication";

    case CloseCode.ServerAtCapacity:
    case CloseCode.RateLimited:
      return "capacity";

    case CloseCode.SessionTimeout:
    case CloseCode.ClientTimeout:
      return "timeout";

    case CloseCode.ProtocolError:
    case CloseCode.InvalidMessage:
    case CloseCode.InvalidPayload:
    case CloseCode.UnsupportedData:
      return "protocol";

    case CloseCode.InternalError:
    case CloseCode.ServiceRestart:
    case CloseCode.TryAgainLater:
    case CloseCode.BadGateway:
      return "server";

    case CloseCode.AbnormalClosure:
    case CloseCode.NoStatus:
    default:
      return "unknown";
  }
}

/**
 * Determines if an error is retryable based on close code.
 */
function isRetryable(code: number): boolean {
  switch (code) {
    // Retryable errors
    case CloseCode.ServerAtCapacity:
    case CloseCode.GoingAway:
    case CloseCode.InternalError:
    case CloseCode.RateLimited:
    case CloseCode.SessionTimeout:
    case CloseCode.ClientTimeout:
    case CloseCode.ServiceRestart:
    case CloseCode.TryAgainLater:
    case CloseCode.AbnormalClosure:
      return true;

    // Non-retryable errors
    case CloseCode.Normal:
    case CloseCode.AuthenticationFailed:
    case CloseCode.InvalidMessage:
    case CloseCode.ProtocolError:
    case CloseCode.ResourceUnavailable:
    case CloseCode.PolicyViolation:
    default:
      return false;
  }
}

/**
 * Gets a human-readable message for a close code.
 */
function getErrorMessage(code: number, reason?: string): string {
  // Use server-provided reason if available
  if (reason && reason.trim()) {
    return reason;
  }

  switch (code) {
    case CloseCode.Normal:
      return "Connection closed normally";
    case CloseCode.GoingAway:
      return "Server is shutting down";
    case CloseCode.ProtocolError:
      return "Protocol error";
    case CloseCode.InternalError:
      return "Server encountered an error";
    case CloseCode.ServerAtCapacity:
      return "Server is at capacity";
    case CloseCode.AuthenticationFailed:
      return "Authentication failed";
    case CloseCode.SessionTimeout:
      return "Session timed out";
    case CloseCode.InvalidMessage:
      return "Invalid message format";
    case CloseCode.RateLimited:
      return "Too many requests";
    case CloseCode.ResourceUnavailable:
      return "Resource not available";
    case CloseCode.ClientTimeout:
      return "Connection timed out";
    case CloseCode.AbnormalClosure:
      return "Connection lost unexpectedly";
    case CloseCode.NoStatus:
      return "Connection closed without status";
    default:
      return `Connection closed (code: ${code})`;
  }
}

/**
 * Gets a detailed description for debugging.
 */
function getErrorDescription(code: number): string {
  switch (code) {
    case CloseCode.Normal:
      return "The connection was closed cleanly by either the client or server.";
    case CloseCode.GoingAway:
      return "The server is going away, either because of a server shutdown or navigating away from the page.";
    case CloseCode.ServerAtCapacity:
      return "The server has no available processing slots. Try again in a few moments.";
    case CloseCode.AuthenticationFailed:
      return "Your authentication credentials are invalid or expired. Please sign in again.";
    case CloseCode.SessionTimeout:
      return "Your session has exceeded the maximum allowed duration.";
    case CloseCode.ClientTimeout:
      return "The server did not receive data from your client within the expected timeframe.";
    case CloseCode.RateLimited:
      return "You have made too many requests. Please wait before trying again.";
    case CloseCode.InvalidMessage:
      return "The client sent a message that the server could not understand.";
    case CloseCode.InternalError:
      return "The server encountered an unexpected error. Our team has been notified.";
    case CloseCode.AbnormalClosure:
      return "The connection was lost unexpectedly. This may be due to network issues.";
    default:
      return `The connection was closed with code ${code}.`;
  }
}

/**
 * Calculates retry delay with exponential backoff.
 */
export function calculateRetryDelay(attemptNumber: number): number {
  const delay = Math.min(
    BASE_RETRY_DELAY_MS * Math.pow(2, attemptNumber),
    MAX_RETRY_DELAY_MS
  );
  // Add jitter (±20%)
  const jitter = delay * 0.2 * (Math.random() * 2 - 1);
  return Math.round(delay + jitter);
}

/**
 * Parses a WebSocket CloseEvent into a structured ConnectionError.
 */
export function parseCloseEvent(event: CloseEvent): ConnectionError {
  const code = event.code;
  const category = getErrorCategory(code);
  const retryable = isRetryable(code);

  return {
    category,
    code,
    message: getErrorMessage(code, event.reason),
    description: getErrorDescription(code),
    retryable,
    retryDelayMs: retryable ? calculateRetryDelay(0) : undefined,
    originalEvent: event,
    timestamp: new Date(),
  };
}

/**
 * Parses a WebSocket error event into a structured ConnectionError.
 */
export function parseErrorEvent(event: Event): ConnectionError {
  return {
    category: "connection",
    message: "Connection error",
    description:
      "An error occurred while connecting to the server. Please check your network connection.",
    retryable: true,
    retryDelayMs: calculateRetryDelay(0),
    originalEvent: event,
    timestamp: new Date(),
  };
}

/**
 * Creates a ConnectionError for a pre-flight check failure.
 */
export function createPreflightError(
  status: "unavailable" | "at_capacity" | "network_error",
  details?: string
): ConnectionError {
  switch (status) {
    case "unavailable":
      return {
        category: "server",
        message: "Server unavailable",
        description:
          details || "The server is not responding. Please try again later.",
        retryable: true,
        retryDelayMs: calculateRetryDelay(0),
        timestamp: new Date(),
      };
    case "at_capacity":
      return {
        category: "capacity",
        code: CloseCode.ServerAtCapacity,
        message: "Server at capacity",
        description:
          details ||
          "The server is currently at capacity. Please try again in a few moments.",
        retryable: true,
        retryDelayMs: calculateRetryDelay(0),
        timestamp: new Date(),
      };
    case "network_error":
      return {
        category: "connection",
        message: "Network error",
        description:
          details ||
          "Could not reach the server. Please check your internet connection.",
        retryable: true,
        retryDelayMs: calculateRetryDelay(0),
        timestamp: new Date(),
      };
  }
}
