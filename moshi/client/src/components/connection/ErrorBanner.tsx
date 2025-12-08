import type { FC } from "react";
import type { ConnectionError } from "../../types/errors";

interface ErrorBannerProps {
  error: ConnectionError;
  onRetry?: () => void;
  onDismiss?: () => void;
  retryCount?: number;
  maxRetries?: number;
}

/**
 * Gets the appropriate icon for an error category.
 */
function getErrorIcon(category: ConnectionError["category"]): string {
  switch (category) {
    case "authentication":
      return "🔒";
    case "capacity":
      return "⏳";
    case "timeout":
      return "⏱️";
    case "connection":
      return "🔌";
    case "protocol":
      return "⚠️";
    case "server":
      return "🔧";
    default:
      return "❌";
  }
}

/**
 * Gets the appropriate background color class for an error category.
 */
function getErrorColorClass(category: ConnectionError["category"]): string {
  switch (category) {
    case "authentication":
      return "bg-red-900/90";
    case "capacity":
      return "bg-yellow-900/90";
    case "timeout":
      return "bg-orange-900/90";
    case "server":
      return "bg-red-800/90";
    default:
      return "bg-gray-800/90";
  }
}

/**
 * Error banner component for displaying connection errors.
 */
export const ErrorBanner: FC<ErrorBannerProps> = ({
  error,
  onRetry,
  onDismiss,
  retryCount = 0,
  maxRetries = 3,
}) => {
  const icon = getErrorIcon(error.category);
  const colorClass = getErrorColorClass(error.category);
  const canRetry = error.retryable && retryCount < maxRetries;

  return (
    <div
      className={`${colorClass} rounded-lg border border-white/20 p-4 shadow-lg`}
      role="alert"
    >
      <div className="flex items-start gap-3">
        <span className="text-2xl" aria-hidden="true">
          {icon}
        </span>
        <div className="flex-1">
          <h3 className="font-semibold text-white">{error.message}</h3>
          <p className="mt-1 text-sm text-white/80">{error.description}</p>

          {error.code && (
            <p className="mt-2 text-xs text-white/60">
              Error code: {error.code}
            </p>
          )}

          <div className="mt-3 flex gap-2">
            {canRetry && onRetry && (
              <button
                type="button"
                onClick={onRetry}
                className="rounded bg-white/20 px-3 py-1 text-sm font-medium text-white hover:bg-white/30 transition-colors"
              >
                Retry {retryCount > 0 && `(${retryCount}/${maxRetries})`}
              </button>
            )}
            {onDismiss && (
              <button
                type="button"
                onClick={onDismiss}
                className="rounded bg-white/10 px-3 py-1 text-sm text-white/80 hover:bg-white/20 transition-colors"
              >
                Dismiss
              </button>
            )}
          </div>

          {error.retryable && error.retryDelayMs && retryCount < maxRetries && (
            <p className="mt-2 text-xs text-white/60">
              {canRetry
                ? `Will retry automatically in ${Math.round(error.retryDelayMs / 1000)}s`
                : "Maximum retry attempts reached"}
            </p>
          )}
        </div>
      </div>
    </div>
  );
};

export default ErrorBanner;
