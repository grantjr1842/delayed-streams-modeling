#!/usr/bin/env bash
# Generates a JWT token and runs the STT client with it
# Usage: ./scripts/run-with-token.sh [mic|file <path>|mic-test]
#
# Environment:
#   ENV                - "development" or "production" (default: development)
#   BETTER_AUTH_SECRET - JWT signing secret (or loaded from .env.{ENV})
#   STT_SERVER_URL     - WebSocket URL (or loaded from .env.{ENV})

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Determine environment
ENV="${ENV:-development}"

# Load environment file
load_env() {
    local env_file="$PROJECT_ROOT/.env.$ENV"
    
    if [[ ! -f "$env_file" ]]; then
        echo "ERROR: $env_file not found" >&2
        echo "Create it with BETTER_AUTH_SECRET and STT_SERVER_URL" >&2
        exit 1
    fi
    
    # Export all variables from the env file
    set -a
    # shellcheck source=/dev/null
    source "$env_file"
    set +a
}

# Generate JWT token using Python (with uv for dependency)
generate_token() {
    local secret="$1"
    uv run --with pyjwt python3 -c "
import jwt
from datetime import datetime, timezone, timedelta

now = datetime.now(timezone.utc)
exp = now + timedelta(hours=1)

claims = {
    'session': {
        'id': 'cli-session',
        'userId': 'cli-user',
        'createdAt': now.isoformat(),
        'updatedAt': now.isoformat(),
        'expiresAt': exp.isoformat(),
        'token': 'cli-token',
        'ipAddress': '127.0.0.1',
        'userAgent': 'kyutai-stt-cli/0.1.0',
    },
    'user': {
        'id': 'cli-user',
        'name': 'CLI User',
        'email': 'cli@local',
        'emailVerified': False,
        'image': None,
    },
    'iat': int(now.timestamp()),
    'exp': int(exp.timestamp()),
}

print(jwt.encode(claims, '$secret', algorithm='HS256'))
"
}

# Main
load_env

# Validate required variables
if [[ -z "${BETTER_AUTH_SECRET:-}" ]]; then
    echo "ERROR: BETTER_AUTH_SECRET not set in .env.$ENV" >&2
    exit 1
fi

if [[ -z "${STT_SERVER_URL:-}" ]]; then
    echo "ERROR: STT_SERVER_URL not set in .env.$ENV" >&2
    exit 1
fi

echo "Environment: $ENV"
echo "Server: $STT_SERVER_URL"

TOKEN=$(generate_token "$BETTER_AUTH_SECRET")

# Pass all arguments to the CLI
cd "$PROJECT_ROOT"
exec cargo run -p kyutai-stt-cli --release -- --url "$STT_SERVER_URL" --auth-token "$TOKEN" "$@"
