#!/usr/bin/env python3
"""Obtain a Better Auth JWT and run kyutai-tts-rs with it.

This script supports two ways to get a token:
1) Login to the Better Auth server (recommended when you have a real user):
   - POST to /api/auth/sign-in/email
   - extract the `better-auth.session_token` cookie (a JWT)

2) Mint a dev JWT locally (convenient for local/dev setups):
   - Sign an HS256 JWT using BETTER_AUTH_SECRET
   - Create a payload compatible with moshi-server's `auth.rs` validation

Note: moshi-server must be started with the same BETTER_AUTH_SECRET for JWT validation.
"""

from __future__ import annotations

import argparse
import base64
import datetime as dt
import hashlib
import hmac
import json
import os
import shutil
import subprocess
import sys
import uuid
import urllib.error
import urllib.request
from http import cookiejar
from pathlib import Path


SESSION_COOKIE_NAME = "better-auth.session_token"


def _b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).decode("utf-8").rstrip("=")


def _utc_now() -> dt.datetime:
    return dt.datetime.now(dt.timezone.utc)


def _to_rfc3339_z(value: dt.datetime) -> str:
    value = value.astimezone(dt.timezone.utc)
    text = value.isoformat(timespec="milliseconds")
    return text.replace("+00:00", "Z")


def mint_dev_jwt(*, secret: str, email: str | None, ttl_minutes: int) -> str:
    now = _utc_now()
    exp = now + dt.timedelta(minutes=ttl_minutes)

    user_id = str(uuid.uuid4())
    session_id = str(uuid.uuid4())

    payload = {
        "session": {
            "id": session_id,
            "userId": user_id,
            "createdAt": _to_rfc3339_z(now),
            "updatedAt": _to_rfc3339_z(now),
            "expiresAt": _to_rfc3339_z(exp),
            "token": None,
            "ipAddress": None,
            "userAgent": "scripts/run_kyutai_tts_rs_with_jwt.py",
        },
        "user": {
            "id": user_id,
            "name": None,
            "email": email,
            "emailVerified": True,
            "image": None,
            "role": "user",
            "status": "approved",
        },
        "iat": int(now.timestamp()),
        "exp": int(exp.timestamp()),
    }

    header = {"alg": "HS256", "typ": "JWT"}

    header_b64 = _b64url(json.dumps(
        header, separators=(",", ":")).encode("utf-8"))
    payload_b64 = _b64url(json.dumps(
        payload, separators=(",", ":")).encode("utf-8"))
    signing_input = f"{header_b64}.{payload_b64}".encode("utf-8")

    sig = hmac.new(secret.encode("utf-8"), signing_input,
                   hashlib.sha256).digest()
    return f"{header_b64}.{payload_b64}.{_b64url(sig)}"


def login_and_get_jwt(*, auth_url: str, login_path: str, email: str, password: str) -> str:
    url = auth_url.rstrip("/") + login_path

    cj = cookiejar.CookieJar()
    opener = urllib.request.build_opener(
        urllib.request.HTTPCookieProcessor(cj))

    body = json.dumps(
        {
            "email": email,
            "password": password,
            "rememberMe": True,
        }
    ).encode("utf-8")

    req = urllib.request.Request(
        url,
        data=body,
        headers={
            "Content-Type": "application/json",
            "Accept": "application/json",
        },
        method="POST",
    )

    try:
        with opener.open(req, timeout=15) as resp:
            _ = resp.read()
    except urllib.error.HTTPError as e:
        detail = None
        try:
            detail = e.read().decode("utf-8", errors="replace")
        except Exception:
            pass
        raise RuntimeError(
            f"Better Auth sign-in failed: HTTP {e.code} {e.reason}\n{detail or ''}".rstrip()
        ) from e
    except urllib.error.URLError as e:
        raise RuntimeError(f"Better Auth sign-in failed: {e}") from e

    for c in cj:
        if c.name == SESSION_COOKIE_NAME:
            return c.value

    raise RuntimeError(
        f"Sign-in succeeded but did not receive cookie {SESSION_COOKIE_NAME!r}."
    )


def redact_token(token: str) -> str:
    if len(token) <= 16:
        return "<redacted>"
    return f"{token[:8]}…{token[-8:]}"


def resolve_repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def build_tts_command(
    *,
    repo_root: Path,
    tts_bin: str,
    input_path: str,
    output_path: str,
    tts_url: str,
    voice: str,
    token: str,
    runs: int,
    json_output: bool,
    seed: int,
    temperature: float,
    top_k: int,
) -> tuple[list[str], Path]:
    args = [
        input_path,
        output_path,
        "--url",
        tts_url,
        "--voice",
        voice,
        "--token",
        token,
        "--runs",
        str(runs),
        "--seed",
        str(seed),
        "--temperature",
        str(temperature),
        "--top-k",
        str(top_k),
    ]
    if json_output:
        args.append("--json")

    if tts_bin == "cargo" or (shutil.which(tts_bin) is None and not Path(tts_bin).exists()):
        manifest = repo_root / "tts-rs" / "Cargo.toml"
        cmd = [
            "cargo",
            "run",
            "--manifest-path",
            str(manifest),
            "--release",
            "--",
            *args,
        ]
        return cmd, repo_root

    cmd = [tts_bin, *args]
    return cmd, repo_root


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Get a Better Auth JWT and run kyutai-tts-rs with it.",
    )

    parser.add_argument("input", help="Input text file path, or '-' for stdin")
    parser.add_argument("output", help="Output WAV file path")

    parser.add_argument(
        "--tts-url",
        default="ws://127.0.0.1:8080",
        help="Moshi server base WebSocket URL (default: ws://127.0.0.1:8080)",
    )
    parser.add_argument(
        "--voice",
        default="expresso/ex03-ex01_happy_001_channel1_334s.wav",
        help="Voice to use (same default as kyutai-tts-rs)",
    )

    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument("--json", action="store_true",
                        help="Print JSON per run")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--temperature", type=float, default=0.8)
    parser.add_argument("--top-k", type=int, default=250)

    parser.add_argument(
        "--token",
        help="Use an existing JWT token (skips login/mint)",
    )

    parser.add_argument(
        "--auth-url",
        default="http://localhost:3001",
        help="Better Auth server base URL (default: http://localhost:3001)",
    )
    parser.add_argument(
        "--login-path",
        default="/api/auth/sign-in/email",
        help="Sign-in endpoint path (default: /api/auth/sign-in/email)",
    )
    parser.add_argument("--email", help="Email for Better Auth sign-in")
    parser.add_argument("--password", help="Password for Better Auth sign-in")

    parser.add_argument(
        "--mint",
        action="store_true",
        help="Force minting a dev JWT locally (requires BETTER_AUTH_SECRET)",
    )
    parser.add_argument(
        "--secret",
        help="JWT secret (defaults to env BETTER_AUTH_SECRET)",
    )
    parser.add_argument(
        "--ttl-minutes",
        type=int,
        default=60,
        help="Minted token TTL in minutes (default: 60)",
    )

    parser.add_argument(
        "--tts-bin",
        default="kyutai-tts-rs",
        help=(
            "TTS client executable to run. Use 'cargo' to run via cargo. "
            "Default: kyutai-tts-rs"
        ),
    )
    parser.add_argument(
        "--no-run",
        action="store_true",
        help="Only print the token (redacted), do not run kyutai-tts-rs",
    )

    return parser.parse_args()


def main() -> int:
    args = parse_args()

    token = args.token
    if token is None:
        if not args.mint and args.email and args.password:
            token = login_and_get_jwt(
                auth_url=args.auth_url,
                login_path=args.login_path,
                email=args.email,
                password=args.password,
            )
        else:
            secret = args.secret or os.environ.get("BETTER_AUTH_SECRET")
            if not secret:
                raise RuntimeError(
                    "Missing BETTER_AUTH_SECRET. Provide --secret or set env BETTER_AUTH_SECRET."
                )
            token = mint_dev_jwt(
                secret=secret,
                email=args.email,
                ttl_minutes=args.ttl_minutes,
            )

    print(f"JWT: {redact_token(token)}")

    if args.no_run:
        return 0

    repo_root = resolve_repo_root()
    cmd, cwd = build_tts_command(
        repo_root=repo_root,
        tts_bin=args.tts_bin,
        input_path=args.input,
        output_path=args.output,
        tts_url=args.tts_url,
        voice=args.voice,
        token=token,
        runs=args.runs,
        json_output=args.json,
        seed=args.seed,
        temperature=args.temperature,
        top_k=args.top_k,
    )

    return subprocess.run(cmd, cwd=str(cwd)).returncode


if __name__ == "__main__":
    raise SystemExit(main())
