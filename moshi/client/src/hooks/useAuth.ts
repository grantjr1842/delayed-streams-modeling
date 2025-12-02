import { useCallback, useEffect, useState } from "react";
import { authClient, getSession } from "../lib/auth-client";

export interface User {
  id: string;
  email: string;
  name: string;
  image?: string;
}

export interface Session {
  id: string;
  token: string;
  userId: string;
  expiresAt: Date;
}

export interface AuthState {
  user: User | null;
  session: Session | null;
  isLoading: boolean;
  isAuthenticated: boolean;
}

/**
 * Hook for managing authentication state in the Moshi client.
 * 
 * Provides:
 * - Current user and session information
 * - Loading state while checking authentication
 * - Sign in/out methods
 * - Session token for API authentication
 */
export function useAuth(): AuthState & {
  signIn: typeof authClient.signIn;
  signUp: typeof authClient.signUp;
  signOut: () => Promise<void>;
  getToken: () => Promise<string | null>;
} {
  const [state, setState] = useState<AuthState>({
    user: null,
    session: null,
    isLoading: true,
    isAuthenticated: false,
  });

  // Check session on mount
  useEffect(() => {
    const checkSession = async () => {
      try {
        const result = await getSession();
        if (result.data) {
          setState({
            user: result.data.user as User,
            session: result.data.session as Session,
            isLoading: false,
            isAuthenticated: true,
          });
        } else {
          setState({
            user: null,
            session: null,
            isLoading: false,
            isAuthenticated: false,
          });
        }
      } catch (error) {
        console.error("Failed to check session:", error);
        setState({
          user: null,
          session: null,
          isLoading: false,
          isAuthenticated: false,
        });
      }
    };

    checkSession();
  }, []);

  const signOut = useCallback(async () => {
    await authClient.signOut();
    setState({
      user: null,
      session: null,
      isLoading: false,
      isAuthenticated: false,
    });
  }, []);

  const getToken = useCallback(async (): Promise<string | null> => {
    if (!state.session) {
      return null;
    }
    return state.session.token;
  }, [state.session]);

  return {
    ...state,
    signIn: authClient.signIn,
    signUp: authClient.signUp,
    signOut,
    getToken,
  };
}
