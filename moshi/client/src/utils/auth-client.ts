import { createAuthClient } from "better-auth/react";

/**
 * Better Auth client instance for the Moshi web client.
 *
 * This client handles:
 * - User authentication (sign in, sign up, sign out)
 * - Session management
 * - Automatic token refresh
 *
 * The client communicates with the Better Auth server at the configured base URL.
 * By default, it uses the same origin as the client (for same-domain deployments).
 *
 * For cross-origin deployments, set VITE_AUTH_URL environment variable.
 */
export const authClient = createAuthClient({
  // Base URL of the auth server
  // If not set, defaults to same origin (e.g., http://localhost:3000/api/auth)
  baseURL: import.meta.env.VITE_AUTH_URL || undefined,
});

// Export commonly used methods for convenience
export const { signIn, signUp, signOut, useSession, getSession } = authClient;

/**
 * Get the current session token for API requests.
 * This can be used to authenticate WebSocket connections or HTTP requests.
 *
 * @returns The session token if authenticated, null otherwise
 */
export async function getSessionToken(): Promise<string | null> {
  const session = await getSession();
  if (!session.data) {
    return null;
  }
  // The session token is stored in cookies by Better Auth
  // For WebSocket auth, we need to pass it as a query parameter
  // since browsers don't allow custom headers on WebSocket connections
  return session.data.session.token;
}
