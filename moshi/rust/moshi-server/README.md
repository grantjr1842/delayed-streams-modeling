# Moshi Server

`moshi-server` is the server implementation for the Moshi voice AI.

## SSL/TLS Support (Secure WebSockets)

`moshi-server` supports Secure WebSockets (WSS) and HTTPS via SSL/TLS.
To enable SSL, you must provide the path to a certificate file (`.pem`) and a private key file (`.pem`).

### Generating Self-Signed Certificates

If you don't have a certificate from a Certificate Authority (CA), you can generate a self-signed certificate for testing/development using `openssl`:

```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes -subj "/CN=localhost"
```

This will create `key.pem` (private key) and `cert.pem` (public certificate).

### Running the Server with SSL

Use the `--ssl-cert` and `--ssl-key` arguments to start the server in secure mode:

```bash
# Example running with cargo
cargo run --release --bin moshi-server -- worker \
    --config config.toml \
    --ssl-cert cert.pem \
    --ssl-key key.pem
```

Or if you have the binary installed:

```bash
moshi-server worker --config config.toml --ssl-cert cert.pem --ssl-key key.pem
```

When SSL is enabled, the server will listen on HTTPS/WSS.
Note: Self-signed certificates will trigger security warnings in browsers. You may need to manually trust the certificate or bypass the warning.

### Default Behavior

If `--ssl-cert` and `--ssl-key` are not provided, the server defaults to plain HTTP/WS.

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

## Automatic SSL (Let's Encrypt)

`moshi-server` supports automatic SSL certificate management via Let's Encrypt (ACME).
To enable this, provide the `--domain` argument.

```bash
moshi-server worker \
    --config config.toml \
    --domain example.com \
    --email admin@example.com \
    --port 443
```

### Arguments
- `--domain <DOMAIN>`: The domain name to obtain a certificate for. Enables ACME mode.
- `--email <EMAIL>`: (Optional) Contact email for Let's Encrypt expiration notices.
- `--acme-cache <DIR>`: (Optional) Directory to store certificates. Defaults to `letsencrypt`.
- `--acme-staging`: (Optional) Use Let's Encrypt Staging environment. Recommended for testing to avoid rate limits.

### Requirements
- The server must be reachable on port 443 (HTTPS) from the public internet for the TLS-ALPN-01 challenge.
- If running on a different port (e.g. 8080), you must forward external port 443 to this port.
- **Note**: Running on port 443 typically requires `sudo` or root privileges on Linux.

```bash
sudo moshi-server worker --config config.toml --domain example.com --port 443
```
