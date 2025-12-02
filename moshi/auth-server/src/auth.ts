import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { db } from "./db";
import * as schema from "./schema";

/**
 * Better Auth server configuration.
 *
 * Uses PostgreSQL with Drizzle ORM for session storage and JWT cookie cache
 * strategy for stateless validation by the Rust moshi-server.
 *
 * The BETTER_AUTH_SECRET environment variable MUST be set and
 * shared with moshi-server for JWT validation.
 */
export const auth = betterAuth({
  // PostgreSQL database with Drizzle adapter
  database: drizzleAdapter(db, {
    provider: "pg",
    schema,
  }),

  // Email/password authentication
  emailAndPassword: {
    enabled: true,
  },

  // Session configuration with JWT cookie cache
  session: {
    cookieCache: {
      enabled: true,
      maxAge: 7 * 24 * 60 * 60, // 7 days cache duration
      strategy: "jwt", // JWT strategy for stateless validation
    },
  },

  // Trust proxy headers (for Caddy reverse proxy)
  trustedOrigins: [
    "https://stt.fullen.dev",
    "http://localhost:5173", // Vite dev server
  ],
});
