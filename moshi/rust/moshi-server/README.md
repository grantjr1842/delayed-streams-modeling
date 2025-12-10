# Moshi Server

`moshi-server` is the server implementation for the Moshi voice AI.

## SSL/TLS (Reverse Proxy)

This server runs on plain HTTP. For production deployments with HTTPS/WSS, use a reverse proxy like [Caddy](https://caddyserver.com/) or nginx to handle SSL termination.

See [REVERSE_PROXY_SETUP.md](../../../docs/REVERSE_PROXY_SETUP.md) for configuration details.

## Authentication

The server uses Better Auth for authentication. Set the `BETTER_AUTH_SECRET` environment variable to the same secret used by your Better Auth server.

```bash
export BETTER_AUTH_SECRET="your-32-character-secret-here"
moshi-server worker --config config.toml ...
```

The server validates JWTs from:
- `Authorization: Bearer <token>` header
- `better-auth.session_token` cookie
- `?token=<jwt>` query parameter (for WebSocket connections)

See [BETTER_AUTH_INTEGRATION.md](../../../docs/BETTER_AUTH_INTEGRATION.md) for detailed setup instructions.
