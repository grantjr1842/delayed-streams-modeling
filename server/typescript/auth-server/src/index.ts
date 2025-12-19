import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { auth } from "./auth";

const app = new Hono();

// CORS configuration for cross-origin requests
app.use(
  "/api/auth/*",
  cors({
    origin: [
      "https://stt.fullen.dev",
      "http://localhost:5173", // Vite dev server
    ],
    credentials: true,
    allowHeaders: ["Content-Type", "Authorization"],
    allowMethods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
  })
);

// Better Auth handler
app.on(["POST", "GET"], "/api/auth/*", (c) => {
  return auth.handler(c.req.raw);
});

// Health check endpoint
app.get("/health", (c) => {
  return c.json({ status: "ok" });
});

const port = parseInt(process.env.AUTH_PORT || "3001");

console.log(`🔐 Auth server starting on port ${port}`);
console.log(
  `   BETTER_AUTH_SECRET: ${process.env.BETTER_AUTH_SECRET ? "✓ set" : "✗ NOT SET"}`
);

serve({
  fetch: app.fetch,
  port,
});

console.log(`🔐 Auth server running at http://localhost:${port}`);
