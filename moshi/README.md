# Deprecated: Moshi Directory

⚠️ **This directory structure is deprecated.**

The Moshi components have been reorganized for better clarity:

## New Locations

- **Rust backend** → [`server/rust/moshi/`](../server/rust/moshi/)
  - moshi-server, moshi-core, moshi-backend, moshi-cli, mimi-pyo3
- **Auth server** → [`server/typescript/auth-server/`](../server/typescript/auth-server/)
- **Python packages** → [`server/python/`](../server/python/)
  - moshi package → [`server/python/moshi/`](../server/python/moshi/)
  - moshi_mlx package → [`server/python/moshi_mlx/`](../server/python/moshi_mlx/)
- **Configs** → [`configs/`](../configs/) (root level)
- **Sample data** → [`audio/`](../audio/) (root level)

Please update your references accordingly. This directory may be removed in a future release.
