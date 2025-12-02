# Plan: Let's Encrypt Support for Moshi Server

## Goal
Enable automatic SSL certificate provisioning via Let's Encrypt using `rustls-acme`.

## Dependencies
- Add `rustls-acme` to `moshi/rust/moshi-server/Cargo.toml`.
- Ensure `tokio` features are enabled in `rustls-acme` (it usually needs `tokio`).

## CLI Arguments
Update `WorkerArgs` in `main.rs`:
- `--domain <DOMAIN>`: The domain name to obtain a certificate for.
- `--email <EMAIL>`: Contact email for Let's Encrypt (optional).
- `--acme-cache <DIR>`: Directory to cache certificates (defaults to `.cache/moshi/acme` or similar).
- `--acme-staging`: Use Let's Encrypt staging environment (for testing).

## Implementation Logic
1.  **Check Arguments**:
    - [x] If `--ssl-cert` and `--ssl-key` are provided: Use Manual SSL (existing).
    - [x] Else if `--domain` is provided: Use ACME SSL.
    - [x] Else: Use Plain HTTP.

2.  **ACME Setup**:
    - [x] Use `rustls_acme::AcmeConfig`.
    - [x] Configure domains, contact email, cache.
    - [x] Configure directory (Production vs Staging).
    - [x] `rustls-acme` handles the ALPN-01 challenge automatically during the TLS handshake.
    - [x] Integrate with `axum-server` via `state.resolver()` and `RustlsConfig::from_config`.
    - [x] Spawn the `rustls-acme` state driver.

## Verification
- [x] Build with `cargo check`. (Passed)
- [ ] Run with `--domain example.com --acme-staging`.
- [ ] Verify it attempts to fetch certs.
