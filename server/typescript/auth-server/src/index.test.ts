import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { auth } from "./auth";

// Create test app with same configuration as production
const createTestApp = () => {
	const app = new Hono();

	app.use(
		"/api/auth/*",
		cors({
			origin: ["http://localhost:5173"],
			credentials: true,
			allowHeaders: ["Content-Type", "Authorization"],
			allowMethods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
		}),
	);

	app.on(["POST", "GET"], "/api/auth/*", (c) => {
		return auth.handler(c.req.raw);
	});

	app.get("/health", (c) => {
		return c.json({ status: "ok" });
	});

	return app;
};

describe("Auth Server", () => {
	const app = createTestApp();

	describe("Health Check", () => {
		it("should return ok status", async () => {
			const res = await app.request("/health");
			expect(res.status).toBe(200);

			const json = await res.json();
			expect(json).toEqual({ status: "ok" });
		});
	});

	describe("CORS", () => {
		it("should allow requests from localhost:5173", async () => {
			const res = await app.request("/api/auth/session", {
				headers: {
					Origin: "http://localhost:5173",
				},
			});

			expect(res.headers.get("Access-Control-Allow-Origin")).toBe("http://localhost:5173");
			expect(res.headers.get("Access-Control-Allow-Credentials")).toBe("true");
		});

		it("should handle OPTIONS preflight requests", async () => {
			const res = await app.request("/api/auth/session", {
				method: "OPTIONS",
				headers: {
					Origin: "http://localhost:5173",
					"Access-Control-Request-Method": "POST",
				},
			});

			expect(res.status).toBe(204);
		});
	});
});
