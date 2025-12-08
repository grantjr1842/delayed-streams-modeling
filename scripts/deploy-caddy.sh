#!/usr/bin/env bash
# Deploy Caddyfile and systemd overrides for Caddy reverse proxy
# Usage: sudo ./scripts/deploy-caddy.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Check if running as root
if [[ $EUID -ne 0 ]]; then
    log_error "This script must be run as root (use sudo)"
    exit 1
fi

# Check if Caddy is installed
if ! command -v caddy &> /dev/null; then
    log_error "Caddy is not installed. Install with: sudo apt install caddy"
    exit 1
fi

log_info "Deploying Caddy configuration..."

# 1. Deploy Caddyfile
log_info "Copying Caddyfile to /etc/caddy/Caddyfile"
cp "$REPO_ROOT/Caddyfile" /etc/caddy/Caddyfile
chown root:root /etc/caddy/Caddyfile
chmod 644 /etc/caddy/Caddyfile

# 2. Deploy systemd override
log_info "Installing systemd override for auto-restart"
mkdir -p /etc/systemd/system/caddy.service.d
cp "$REPO_ROOT/systemd/caddy.service.d/override.conf" /etc/systemd/system/caddy.service.d/override.conf
chown root:root /etc/systemd/system/caddy.service.d/override.conf
chmod 644 /etc/systemd/system/caddy.service.d/override.conf

# 3. Reload systemd
log_info "Reloading systemd daemon"
systemctl daemon-reload

# 4. Validate Caddyfile
log_info "Validating Caddyfile syntax"
if ! caddy validate --config /etc/caddy/Caddyfile; then
    log_error "Caddyfile validation failed!"
    exit 1
fi

# 5. Reload Caddy (graceful reload, no downtime)
log_info "Reloading Caddy service"
systemctl reload caddy || systemctl restart caddy

# 6. Verify service status
log_info "Verifying Caddy service status"
if systemctl is-active --quiet caddy; then
    log_info "Caddy is running"
else
    log_error "Caddy failed to start!"
    systemctl status caddy --no-pager
    exit 1
fi

# 7. Show restart configuration
log_info "Restart configuration:"
systemctl show caddy --property=Restart,RestartSec,WatchdogSec

log_info "Deployment complete!"
echo ""
echo "Domains configured:"
echo "  - https://transcribe.fullen.dev"
echo "  - https://stt.fullen.dev"
echo ""
echo "To verify SSL certificates:"
echo "  curl -sI https://transcribe.fullen.dev | grep -i server"
echo "  curl -sI https://stt.fullen.dev | grep -i server"
