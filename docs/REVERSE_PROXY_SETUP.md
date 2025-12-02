# Reverse Proxy Setup with Caddy

This guide describes how to set up Caddy as a reverse proxy for the Moshi Server. This allows accessing the server via a secure HTTPS domain (`stt.fullen.dev`) while the server itself runs on a non-privileged port (`8080`) locally.

## Prerequisites

- Moshi Server installed and running.
- Caddy installed (`sudo apt install caddy`).
- Domain `stt.fullen.dev` DNS pointing to this server.
- Ports 80 and 443 open on the firewall.

## Routes Handled

The following routes are proxied to moshi-server:

| Route | Type | Description |
|-------|------|-------------|
| `/api/chat` | WebSocket | Real-time audio streaming (ASR) |
| `/api/asr` | WebSocket | ASR streaming (dynamic path based on config) |
| `/api/tts` | POST | Text-to-speech endpoint |
| `/api/tts_streaming` | WebSocket | TTS streaming |
| `/api/build_info` | GET | Server build information |
| `/api/modules_info` | GET | Module information |
| `/metrics` | GET | Prometheus metrics |
| `/*` (fallback) | Static | Client application files |

## Configuration

The Caddy configuration is located in the `Caddyfile` in the repository root:

```caddy
stt.fullen.dev {
    reverse_proxy localhost:8080
}
```

### With Auth Server (Optional)

If you're running the Better Auth server for user authentication:

```caddy
stt.fullen.dev {
    # Auth server routes
    handle /api/auth/* {
        reverse_proxy localhost:3001
    }
    handle /health {
        reverse_proxy localhost:3001
    }

    # Everything else goes to moshi-server
    handle {
        reverse_proxy localhost:8080
    }
}
```

## Running

1. **Start Moshi Server**:
   Run moshi-server on the default port 8080 (HTTP). Do NOT use `--ssl-cert` or `--domain` flags.

   ```bash
   moshi-server worker --config config.toml
   ```

2. **Start Auth Server (Optional)**:
   If using Better Auth for authentication:

   ```bash
   cd moshi/auth-server
   pnpm dev  # Runs on port 3001
   ```

3. **Start Caddy**:
   
   ```bash
   # Run in foreground (useful for testing)
   sudo caddy run

   # Or start as a background service
   sudo caddy start
   ```

Caddy will automatically provision and renew SSL certificates for `stt.fullen.dev` via Let's Encrypt.

## Verification

Test that all routes are working:

```bash
# Test static files (client app)
curl -I https://stt.fullen.dev/

# Test API endpoint
curl https://stt.fullen.dev/api/build_info

# Test metrics
curl https://stt.fullen.dev/metrics

# Test WebSocket (requires wscat or similar)
# wscat -c wss://stt.fullen.dev/api/chat
```

## Troubleshooting

### WebSocket Connection Issues

Caddy automatically handles WebSocket upgrades. If you experience issues:

1. Ensure moshi-server is running and accessible on port 8080
2. Check Caddy logs: `sudo journalctl -u caddy -f`
3. Verify the WebSocket endpoint responds: `curl -I -H "Upgrade: websocket" https://stt.fullen.dev/api/chat`

### SSL Certificate Issues

Caddy automatically provisions certificates. If there are issues:

1. Ensure ports 80 and 443 are open
2. Verify DNS is pointing to the correct IP
3. Check Caddy logs for ACME errors
