import { describe, it, expect } from "vitest";
import { Hono } from "hono";
import { cors } from "hono/cors";

// Create a minimal test app that mirrors the production app structure
// without requiring database connections
const createTestApp = () => {
	const app = new Hono();

	// CORS middleware
	app.use(
		"/api/auth/*",
		cors({
			origin: ["https://stt.fullen.dev", "http://localhost:5173"],
			credentials: true,
			allowHeaders: ["Content-Type", "Authorization"],
			allowMethods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
		}),
	);

	// Mock auth endpoints for testing route handling
	app.post("/api/auth/sign-up", async (c) => {
		const body = await c.req.json().catch(() => ({}));
		if (!body.email || !body.password) {
			return c.json({ error: "Email and password required" }, 400);
		}
		if (!body.email.includes("@")) {
			return c.json({ error: "Invalid email format" }, 400);
		}
		if (body.password.length < 8) {
			return c.json({ error: "Password must be at least 8 characters" }, 400);
		}
		return c.json({ success: true, userId: "test-user-id" }, 201);
	});

	app.post("/api/auth/sign-in", async (c) => {
		const body = await c.req.json().catch(() => ({}));
		if (!body.email || !body.password) {
			return c.json({ error: "Email and password required" }, 400);
		}
		return c.json({ success: true, token: "mock-jwt-token" }, 200);
	});

	app.get("/api/auth/session", (c) => {
		const authHeader = c.req.header("Authorization");
		if (!authHeader) {
			return c.json({ session: null }, 200);
		}
		return c.json({
			session: { userId: "test-user-id", expiresAt: new Date(Date.now() + 86400000).toISOString() },
		});
	});

	app.post("/api/auth/sign-out", (c) => {
		return c.json({ success: true }, 200);
	});

	app.get("/health", (c) => {
		return c.json({ status: "ok" });
	});

	return app;
};

describe("Auth Server Routes", () => {
	const app = createTestApp();

	describe("Sign Up", () => {
		it("should reject signup without email", async () => {
			const res = await app.request("/api/auth/sign-up", {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ password: "password123" }),
			});
			expect(res.status).toBe(400);
			const json = await res.json();
			expect(json.error).toContain("Email and password required");
		});

		it("should reject signup without password", async () => {
			const res = await app.request("/api/auth/sign-up", {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ email: "test@example.com" }),
			});
			expect(res.status).toBe(400);
		});

		it("should reject invalid email format", async () => {
			const res = await app.request("/api/auth/sign-up", {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ email: "invalid-email", password: "password123" }),
			});
			expect(res.status).toBe(400);
			const json = await res.json();
			expect(json.error).toContain("Invalid email");
		});

		it("should reject short passwords", async () => {
			const res = await app.request("/api/auth/sign-up", {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ email: "test@example.com", password: "short" }),
			});
			expect(res.status).toBe(400);
			const json = await res.json();
			expect(json.error).toContain("8 characters");
		});

		it("should accept valid signup", async () => {
			const res = await app.request("/api/auth/sign-up", {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ email: "test@example.com", password: "password123" }),
			});
			expect(res.status).toBe(201);
			const json = await res.json();
			expect(json.success).toBe(true);
			expect(json.userId).toBeDefined();
		});
	});

	describe("Sign In", () => {
		it("should reject signin without credentials", async () => {
			const res = await app.request("/api/auth/sign-in", {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({}),
			});
			expect(res.status).toBe(400);
		});

		it("should accept valid signin", async () => {
			const res = await app.request("/api/auth/sign-in", {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ email: "test@example.com", password: "password123" }),
			});
			expect(res.status).toBe(200);
			const json = await res.json();
			expect(json.token).toBeDefined();
		});
	});

	describe("Session", () => {
		it("should return null session without auth header", async () => {
			const res = await app.request("/api/auth/session");
			expect(res.status).toBe(200);
			const json = await res.json();
			expect(json.session).toBeNull();
		});

		it("should return session with auth header", async () => {
			const res = await app.request("/api/auth/session", {
				headers: { Authorization: "Bearer mock-token" },
			});
			expect(res.status).toBe(200);
			const json = await res.json();
			expect(json.session).toBeDefined();
			expect(json.session.userId).toBe("test-user-id");
		});
	});

	describe("Sign Out", () => {
		it("should successfully sign out", async () => {
			const res = await app.request("/api/auth/sign-out", { method: "POST" });
			expect(res.status).toBe(200);
			const json = await res.json();
			expect(json.success).toBe(true);
		});
	});

	describe("CORS", () => {
		it("should allow production origin", async () => {
			const res = await app.request("/api/auth/session", {
				headers: { Origin: "https://stt.fullen.dev" },
			});
			expect(res.headers.get("Access-Control-Allow-Origin")).toBe("https://stt.fullen.dev");
		});

		it("should allow localhost dev origin", async () => {
			const res = await app.request("/api/auth/session", {
				headers: { Origin: "http://localhost:5173" },
			});
			expect(res.headers.get("Access-Control-Allow-Origin")).toBe("http://localhost:5173");
		});
	});
});
