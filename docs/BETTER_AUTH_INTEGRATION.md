# Better Auth Integration

This document describes how to integrate Better Auth for user authentication with the moshi-server and web client.

## Overview

The moshi-server supports multiple authentication methods:

1. **Legacy API Key** - Simple API key via `kyutai-api-key` header or `auth_id` query parameter
2. **Better Auth JWT** - JWT tokens from Better Auth's cookie cache feature
3. **Session Cookie** - Better Auth session cookies (for same-origin deployments)

## Server Configuration

### Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `MOSHI_API_KEY` | Comma-separated list of valid API keys | No |
| `BETTER_AUTH_SECRET` | Shared secret with Better Auth for JWT validation | No (but required for JWT auth) |

### Example

```bash
export MOSHI_API_KEY="key1,key2,key3"
export BETTER_AUTH_SECRET="your-32-character-secret-here"

moshi-server worker --config configs/config-stt-en-hf.toml
```

## Better Auth Server Configuration

Configure Better Auth with JWT cookie caching to enable stateless validation:

```typescript
// auth.ts
import { betterAuth } from "better-auth";

export const auth = betterAuth({
  // Your database configuration
  database: { /* ... */ },
  
  // Enable email/password auth
  emailAndPassword: {
    enabled: true,
  },
  
  // Configure session with JWT cookie cache
  session: {
    cookieCache: {
      enabled: true,
      maxAge: 5 * 60, // 5 minutes
      strategy: "jwt", // Use JWT for stateless validation
    },
  },
});
```

**Important**: The `BETTER_AUTH_SECRET` environment variable must be the same on both the Better Auth server and the moshi-server.

## Web Client Configuration

### Installation

```bash
cd moshi/client
npm install better-auth
```

### Auth Client Setup

The auth client is configured in `src/lib/auth-client.ts`:

```typescript
import { createAuthClient } from "better-auth/react";

export const authClient = createAuthClient({
  baseURL: import.meta.env.VITE_AUTH_URL || undefined,
});

export const { signIn, signUp, signOut, useSession, getSession } = authClient;
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `VITE_AUTH_URL` | Base URL of the Better Auth server | Same origin |

### Using Authentication in Components

```typescript
import { useAuth } from "../hooks/useAuth";

function MyComponent() {
  const { isAuthenticated, isLoading, user, signIn, signOut, getToken } = useAuth();

  if (isLoading) {
    return <div>Loading...</div>;
  }

  if (!isAuthenticated) {
    return (
      <button onClick={() => signIn.email({ email, password })}>
        Sign In
      </button>
    );
  }

  return (
    <div>
      <p>Welcome, {user?.name}!</p>
      <button onClick={signOut}>Sign Out</button>
    </div>
  );
}
```

### WebSocket Authentication

The session token is automatically passed to WebSocket connections via the `auth_id` query parameter:

```typescript
// In Conversation.tsx
const WSURL = buildURL({
  workerAddr,
  params: modelParams,
  workerAuthId,
  sessionToken, // Better Auth JWT token
  email,
  textSeed,
  audioSeed,
});
```

## Authentication Flow

```
┌─────────────┐     ┌─────────────────┐     ┌──────────────┐
│  Web Client │     │  Better Auth    │     │ moshi-server │
│             │     │     Server      │     │              │
└──────┬──────┘     └────────┬────────┘     └──────┬───────┘
       │                     │                      │
       │  1. Sign In         │                      │
       │────────────────────>│                      │
       │                     │                      │
       │  2. JWT Cookie      │                      │
       │<────────────────────│                      │
       │                     │                      │
       │  3. WebSocket + JWT │                      │
       │───────────────────────────────────────────>│
       │                     │                      │
       │                     │  4. Validate JWT     │
       │                     │  (using shared       │
       │                     │   BETTER_AUTH_SECRET)│
       │                     │                      │
       │  5. Authenticated   │                      │
       │<───────────────────────────────────────────│
       │                     │                      │
```

## JWT Claims Structure

The moshi-server expects the following claims in the Better Auth JWT:

```json
{
  "id": "session-id",
  "userId": "user-id",
  "createdAt": 1234567890,
  "updatedAt": 1234567890,
  "expiresAt": 1234567890,
  "ipAddress": "127.0.0.1",
  "userAgent": "Mozilla/5.0...",
  "iat": 1234567890,
  "exp": 1234567890
}
```

## Backward Compatibility

The legacy API key authentication continues to work alongside Better Auth:

- API keys can be passed via `kyutai-api-key` header
- API keys can be passed via `auth_id` query parameter
- Both methods are checked before JWT validation

This allows programmatic API access while web users authenticate via Better Auth.

## Troubleshooting

### JWT Validation Failed

1. Ensure `BETTER_AUTH_SECRET` is set on both servers
2. Verify the secret is at least 32 characters
3. Check that the JWT hasn't expired

### Session Cookie Not Sent

1. Ensure the web client and moshi-server are on the same domain (or use CORS)
2. Check that cookies are enabled in the browser
3. Verify the `SameSite` cookie attribute is appropriate

### WebSocket Connection Rejected

1. Check the browser console for authentication errors
2. Verify the session token is being passed correctly
3. Ensure the user is signed in before connecting

## Security Considerations

1. **HTTPS Required**: Always use HTTPS in production to protect tokens in transit
2. **Secret Management**: Store `BETTER_AUTH_SECRET` securely (e.g., environment variables, secrets manager)
3. **Token Expiration**: Configure appropriate session expiration times
4. **CORS**: Configure CORS properly for cross-origin deployments
