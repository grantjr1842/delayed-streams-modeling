
# AGENTS

## Python

Keep interaction focused and concise. When editing or running code, prefer repository conventions and project README instructions.

Python + uv best practices (see Astral docs):
- Install/manage Python with uv: prefer `uv python install 3.12` (or the repo’s pinned version) before working.
- Projects: rely on `pyproject.toml`/`uv.lock` and run `uv sync` to create/sync the env; use `uv sync --frozen` when reproducibility matters (CI, releases).
- Shell/commands: use `uv run …` so commands execute in the project env (e.g., `uv run python scripts/foo.py`, `uv run pytest`); use `uv shell` if you need an interactive env.
- Scripts: for one-off scripts with deps, use `uv run script.py -- arg1 arg2`; do not pip/pipenv/venv directly.
- When unsure of project-specific flags or aliases, check the repository README or contribution docs before running commands.
