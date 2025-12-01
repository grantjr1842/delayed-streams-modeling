# Reverse Proxy Setup with Caddy

This guide describes how to set up Caddy as a reverse proxy for the Moshi Server. This allows accessing the server via a secure HTTPS domain (`stt.fullen.dev`) while the server itself runs on a non-privileged port (`8080`) locally.

## Prerequisites

- Moshi Server installed and running.
- Caddy installed (`sudo apt install caddy`).
- Domain `stt.fullen.dev` DNS pointing to this server.
- Ports 80 and 443 open on the firewall.

## Configuration

The Caddy configuration is located in the `Caddyfile` in the repository root:

```caddy
stt.fullen.dev {
    reverse_proxy localhost:8080
}
```

## Running

1. **Start Moshi Server**:
   Run moshi-server on the default port 8080 (HTTP). Do NOT use `--ssl-cert` or `--domain` flags.

   ```bash
   moshi-server worker --config config.toml
   ```

2. **Start Caddy**:
   
   ```bash
   # Run in foreground (useful for testing)
   sudo caddy run

   # Or start as a background service
   sudo caddy start
   ```

Caddy will automatically provision and renew SSL certificates for `stt.fullen.dev` via Let's Encrypt.
