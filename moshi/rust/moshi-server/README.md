# Moshi Server

`moshi-server` is the server implementation for the Moshi voice AI.

## SSL/TLS (Reverse Proxy)

This server runs on plain HTTP. For production deployments with HTTPS/WSS, use a reverse proxy like [Caddy](https://caddyserver.com/) or nginx to handle SSL termination.

See [REVERSE_PROXY_SETUP.md](../../../docs/REVERSE_PROXY_SETUP.md) for configuration details.

## Authentication

The server supports multiple authentication methods:

1. **API Key** - Simple API key via header or query parameter
2. **Better Auth JWT** - JWT tokens from Better Auth's cookie cache feature
3. **Session Cookie** - Better Auth session cookies

### API Key Authentication

Set the `MOSHI_API_KEY` environment variable to a comma-separated list of authorized keys.

```bash
export MOSHI_API_KEY="secret_token_1,secret_token_2"
moshi-server worker --config config.toml ...
```

API keys can be passed via:
- `kyutai-api-key` HTTP header
- `auth_id` query parameter

### Better Auth JWT Authentication

For web applications using [Better Auth](https://www.better-auth.com/), set the `BETTER_AUTH_SECRET` environment variable to the same secret used by your Better Auth server.

```bash
export BETTER_AUTH_SECRET="your-32-character-secret-here"
moshi-server worker --config config.toml ...
```

The server validates JWTs from:
- `Authorization: Bearer <token>` header
- `better-auth.session_token` cookie
- `auth_id` query parameter (for WebSocket connections)

See [BETTER_AUTH_INTEGRATION.md](../../../docs/BETTER_AUTH_INTEGRATION.md) for detailed setup instructions.

### Configuration File

You can also set `authorized_ids` in the configuration file, but using environment variables is recommended to avoid hardcoding secrets.

```toml
authorized_ids = ["secret_token_1"]
```
