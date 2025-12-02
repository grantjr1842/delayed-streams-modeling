# SSL Implementation Plan for Moshi Server

## Goal
Document and implement secure web sockets (WSS) via SSL for `moshi-server`.

## Current State
- `moshi-server` uses `axum` 0.8.7.
- Server binds to HTTP/TCP port 8080 (default).
- No SSL/TLS support currently.
- `axum-server` is defined in workspace dependencies but not used in `moshi-server`.

## Requirements
- Add CLI arguments for SSL certificate and key paths: `--ssl-cert` and `--ssl-key`.
- If these arguments are provided, the server should start in HTTPS/WSS mode.
- If not provided, it falls back to HTTP/WS.
- Document how to generate self-signed certs or use real ones.

## Implementation Steps
1.  **Dependency Management**:
    - [x] Add `axum-server = { workspace = true }` to `moshi/rust/moshi-server/Cargo.toml`.
    - [x] Verify compatibility between `axum-server` 0.7 and `axum` 0.8. (Verified via `cargo check`).

2.  **Code Changes (`moshi-server/src/main.rs`)**:
    - [x] Update `WorkerArgs` struct to include optional `ssl_cert` and `ssl_key` fields.
    - [x] In `main_`, check if SSL args are present.
    - [x] If present, load config and bind using SSL.
    - [x] Refactor the server startup to handle both HTTP and HTTPS paths.

3.  **Documentation**:
    - [x] Update `README.md` or create `docs/SSL_SETUP.md`. (Created `moshi/rust/moshi-server/README.md`)
    - [x] Provide examples of `openssl` commands to generate certs.

## Verification
- [x] Build with `cargo check` / `cargo build`. (Passed `cargo check`)
- [ ] Test with self-signed certs. (User can verify)
- [ ] Connect using a WSS client (or browser).
