# TTS Server + Client Test Results

Date: 2025-12-21

## Summary
Attempted to run the TTS server with `configs/tts/config-tts-fast.toml` and execute the
smoke test, but the server did not become healthy within the initial wait window.
No HTTP or WebSocket TTS requests were executed yet.

## Environment
- Binary: `target/debug/moshi-server`
- Config: `configs/tts/config-tts-fast.toml`
- Auth: Better Auth JWT validation enabled (`BETTER_AUTH_SECRET` is set)

## Steps Attempted
1) Started server:

```bash
./target/debug/moshi-server worker --config configs/tts/config-tts-fast.toml
```

2) Waited up to ~60 seconds for `http://127.0.0.1:8080/api/health` to respond.

Result:
- Server did not respond within the wait window.
- No HTTP or WebSocket TTS requests were sent.

## Logs Captured
- `logs/tts-test-20251221T150350Z.log`
- `logs/moshi-server-rust/tts-fast/log.config-tts-fast`

Notable log lines:
- Build info emitted successfully.
- Better Auth JWT validation enabled.
- GPU capability detection warning: "Could not detect GPU capabilities. Using configured values."
- Worker start initiated (no further readiness logs within 30s).

## Blockers / Open Items
- Server startup likely requires additional time for model downloads or warmup.
- Auth is enabled; a valid JWT token is required to test `/api/tts` and `/api/tts_streaming`.
- If startup continues to stall, confirm GPU availability or run with a CPU-friendly configuration.

## Next Actions
- Wait longer for server readiness (model downloads may be large).
- Generate a JWT token (see `client/rust/kyutai-stt-cli/scripts/generate_test_token.py`) and re-run:
  `tools/tts-smoke-test.sh --token <JWT>`.
