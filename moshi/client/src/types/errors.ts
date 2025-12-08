/**
 * WebSocket close codes used by moshi-server.
 * Standard codes (1000-1015) are defined by RFC 6455.
 * Custom application codes (4000-4999) are defined by the server.
 */
export enum CloseCode {
  // Standard RFC 6455 codes
  Normal = 1000,
  GoingAway = 1001,
  ProtocolError = 1002,
  UnsupportedData = 1003,
  NoStatus = 1005,
  AbnormalClosure = 1006,
  InvalidPayload = 1007,
  PolicyViolation = 1008,
  MessageTooBig = 1009,
  MandatoryExtension = 1010,
  InternalError = 1011,
  ServiceRestart = 1012,
  TryAgainLater = 1013,
  BadGateway = 1014,
  TlsHandshake = 1015,

  // Custom moshi-server codes (4000-4999)
  ServerAtCapacity = 4000,
  AuthenticationFailed = 4001,
  SessionTimeout = 4002,
  InvalidMessage = 4003,
  RateLimited = 4004,
  ResourceUnavailable = 4005,
  ClientTimeout = 4006,
}

/**
 * Error categories for UI display and retry logic.
 */
export type ErrorCategory =
  | "connection"
  | "authentication"
  | "capacity"
  | "timeout"
  | "protocol"
  | "server"
  | "unknown";

/**
 * Structured connection error with all relevant information.
 */
export interface ConnectionError {
  /** Error category for UI display */
  category: ErrorCategory;
  /** WebSocket close code (if available) */
  code?: number;
  /** Human-readable error message */
  message: string;
  /** Detailed description for debugging */
  description: string;
  /** Whether the client should retry the connection */
  retryable: boolean;
  /** Suggested retry delay in milliseconds (if retryable) */
  retryDelayMs?: number;
  /** Original error event (if available) */
  originalEvent?: Event | CloseEvent;
  /** Timestamp when error occurred */
  timestamp: Date;
}

/**
 * Server status response from /api/status endpoint.
 */
export interface ServerStatus {
  status: "healthy" | "degraded" | "unhealthy";
  uptime_seconds: number;
  started_at: string;
  build: {
    build_timestamp: string;
    git_hash: string;
    rustc_version: string;
    [key: string]: string;
  };
  capacity: {
    total_slots: number;
    used_slots: number;
    available_slots: number;
    modules: Array<{
      name: string;
      module_type: string;
      total_slots: number;
      used_slots: number;
      available_slots: number;
    }>;
  };
  auth: {
    api_key_configured: boolean;
    better_auth_enabled: boolean;
  };
}

/**
 * Health check response from /api/health endpoint.
 */
export interface HealthResponse {
  status: "ok" | "error";
  uptime_seconds: number;
}
