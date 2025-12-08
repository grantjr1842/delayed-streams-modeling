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
    reverse_proxy localhost:8080 {
        # Explicit WebSocket header passthrough for reliable upgrades
        header_up Connection {http.request.header.Connection}
        header_up Upgrade {http.request.header.Upgrade}
    }
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
        reverse_proxy localhost:8080 {
            header_up Connection {http.request.header.Connection}
            header_up Upgrade {http.request.header.Upgrade}
        }
    }
}
```

## Deployment

### Quick Deploy

Run the deployment script to install/update the Caddyfile and systemd configuration:

```bash
sudo ./scripts/deploy-caddy.sh
```

This script:
1. Copies `Caddyfile` to `/etc/caddy/Caddyfile`
2. Installs systemd override for auto-restart on failure
3. Validates the Caddyfile syntax
4. Reloads Caddy gracefully (no downtime)

### Manual Deployment

If you prefer manual steps:

```bash
# Copy Caddyfile
sudo cp Caddyfile /etc/caddy/Caddyfile

# Install systemd override for auto-restart
sudo mkdir -p /etc/systemd/system/caddy.service.d
sudo cp systemd/caddy.service.d/override.conf /etc/systemd/system/caddy.service.d/

# Reload systemd and Caddy
sudo systemctl daemon-reload
sudo caddy validate --config /etc/caddy/Caddyfile
sudo systemctl reload caddy
```

## Systemd Service Management

Caddy runs as a systemd service with auto-restart enabled:

```bash
# Check status
sudo systemctl status caddy

# View logs
sudo journalctl -u caddy -f

# Restart service
sudo systemctl restart caddy

# Enable on boot (usually already enabled)
sudo systemctl enable caddy
```

### Auto-Restart Configuration

The systemd override (`/etc/systemd/system/caddy.service.d/override.conf`) configures:
- **Restart=on-failure**: Automatically restart if Caddy crashes
- **RestartSec=5s**: Wait 5 seconds before restarting
- **WatchdogSec=30s**: Restart if Caddy becomes unresponsive

Verify the configuration:
```bash
systemctl show caddy --property=Restart,RestartSec,WatchdogSec
# Expected: Restart=on-failure, RestartSec=5s, WatchdogSec=30s
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

3. **Ensure Caddy is running**:
   
   Caddy should already be running as a systemd service. Verify with:
   ```bash
   sudo systemctl status caddy
   ```

   If not running, start it:
   ```bash
   sudo systemctl start caddy
   ```

Caddy automatically provisions and renews SSL certificates for both `transcribe.fullen.dev` and `stt.fullen.dev` via Let's Encrypt. Certificates are stored in `/var/lib/caddy/.local/share/caddy/certificates/`.

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

Caddy automatically handles WebSocket upgrades, but explicit header passthrough is configured for reliability:

```caddy
header_up Connection {http.request.header.Connection}
header_up Upgrade {http.request.header.Upgrade}
```

If you experience issues:

1. Ensure moshi-server is running and accessible on port 8080
2. Check Caddy logs: `sudo journalctl -u caddy -f`
3. Verify the WebSocket endpoint responds: `curl -I -H "Upgrade: websocket" https://stt.fullen.dev/api/chat`
4. Reload Caddy after config changes: `sudo caddy reload`

### SSL Certificate Issues

Caddy automatically provisions certificates. If there are issues:

1. Ensure ports 80 and 443 are open
2. Verify DNS is pointing to the correct IP
3. Check Caddy logs for ACME errors
