#!/usr/bin/env python3
"""
Generate a test JWT token for moshi-server authentication.

Usage:
    uv run scripts/generate_test_token.py
    
    # Or with custom expiry (in hours)
    uv run scripts/generate_test_token.py --hours 24
"""
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "pyjwt",
# ]
# ///

import argparse
import base64
import os
from datetime import datetime, timezone, timedelta

import jwt


def load_secret() -> str:
    """Load BETTER_AUTH_SECRET from .env file or environment."""
    # Try environment first
    secret = os.environ.get("BETTER_AUTH_SECRET")
    if secret:
        return secret
    
    # Try .env file
    env_path = os.path.join(os.path.dirname(__file__), "..", ".env")
    if os.path.exists(env_path):
        with open(env_path) as f:
            for line in f:
                line = line.strip()
                if line.startswith("BETTER_AUTH_SECRET="):
                    return line.split("=", 1)[1].strip().strip('"').strip("'")
    
    raise ValueError("BETTER_AUTH_SECRET not found in environment or .env file")


def generate_token(secret: str, hours: float = 1.0) -> str:
    """Generate a JWT token matching Better Auth's expected format."""
    now = datetime.now(timezone.utc)
    exp = now + timedelta(hours=hours)
    
    # Match the BetterAuthClaims structure from moshi-server/src/auth.rs
    claims = {
        "session": {
            "id": "test-session-id",
            "userId": "test-user-id",
            "createdAt": now.isoformat(),
            "updatedAt": now.isoformat(),
            "expiresAt": exp.isoformat(),
            "token": "test-session-token",
            "ipAddress": "127.0.0.1",
            "userAgent": "kyutai-tts-rs/0.1.0",
        },
        "user": {
            "id": "test-user-id",
            "name": "Test User",
            "email": "test@example.com",
            "emailVerified": False,
            "image": None,
        },
        "iat": int(now.timestamp()),
        "exp": int(exp.timestamp()),
    }
    
    # Better Auth uses HS256 with the raw secret
    token = jwt.encode(claims, secret, algorithm="HS256")
    return token


def main():
    parser = argparse.ArgumentParser(description="Generate a test JWT token for moshi-server")
    parser.add_argument("--hours", type=float, default=1.0, help="Token validity in hours (default: 1)")
    args = parser.parse_args()
    
    try:
        secret = load_secret()
        token = generate_token(secret, args.hours)
        
        print("Generated test JWT token:")
        print()
        print(token)
        print()
        print("Usage with tts-rs client:")
        print(f'  echo "Hello world" | cargo run --release -- - output.wav --token "{token}"')
        print()
        
    except ValueError as e:
        print(f"Error: {e}")
        return 1
    
    return 0


if __name__ == "__main__":
    exit(main())
