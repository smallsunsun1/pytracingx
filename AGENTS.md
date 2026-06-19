# AGENTS.md

This file provides guidance to the AI agent when working with code in this repository.

## Project layout

- Rust crate at the repo root (`Cargo.toml`, `rust/src/`), built as a `cdylib` named `_native`.
- Python package at `python/pytracingx/` (configured via `python-source = "python"` in `pyproject.toml`).
- Native module is exposed as `pytracingx._native`; the public Python API re-exports from there.

## Build and test

Local dev workflow uses `uv build` + reinstall (not `maturin develop`):

```bash
uv build --wheel
pip install --force-reinstall --no-deps dist/pytracingx-*.whl
pytest -v
```

`/verify` runs the full sequence end-to-end. Use it before declaring a change "done".

## Tests

- The Rust `tracing` dispatcher is **one-shot per process**. `tests/conftest.py` initializes pytracingx exactly once per session and reuses that runtime; tests cannot re-init with a different config mid-process. Don't add tests that require a clean uninitialized state — they will poison the dispatcher for everything after.
- `pytest-asyncio` is in `auto` mode; async tests don't need a marker.

## Style

- Ruff is the only linter/formatter (`ruff check`, `ruff format`). Config in `pyproject.toml`: line-length 100, target py39, rules `E F I UP B W`. `.pyi` files are exempt from `E501`.
- Pre-commit hook auto-fixes on commit (`.pre-commit-config.yaml`). CI also runs `ruff check` and `ruff format --check` on `python/pytracingx/` and `tests/`.

## Versioning and releases

- Version must be identical in `pyproject.toml`, `Cargo.toml`, and the `v*` git tag. `release.yml`'s `verify-version` job fails the build if any drift.
- The AI may bump version numbers in `pyproject.toml` + `Cargo.toml` (and update `Cargo.lock` via `cargo update -p pytracingx --precise <ver>`) when asked, but **never pushes git tags** — the user owns release triggers.
- Use `/bump-version <version>` for the bump.

## Commit messages

Conventional Commits style: `feat:`, `fix:`, `chore:`, `ci:`, `docs:`, `test:`, `refactor:`. Keep the subject line under ~70 chars; details in the body when needed.

## Gotchas

- **`Cargo.lock` is gitignored.** Don't try to commit it.
- **`openssl-sys` uses the `vendored` feature.** OpenSSL is statically compiled into the wheel; CI release containers install `perl` + `make` + `pkg-config` for that build. Don't switch to system OpenSSL.
- **Rust edition is 2024**, requires rustc ≥ 1.85 (despite the README saying 1.75).
- **`run.sh`** is gitignored and contains hardcoded Aliyun ARMS credentials. Treat it as developer-local; don't commit it or echo its contents to logs.
- **Release workflow triggers on tag push, GitHub Release publish, AND `workflow_dispatch`.** Don't push `v*` tags casually.
