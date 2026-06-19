---
name: verify
description: Verify a change works by rebuilding the native extension wheel, reinstalling it, then running the full test suite and ruff checks. Use after making non-trivial code changes to confirm correctness before declaring done.
---

Run this exact sequence from the repo root and report any failures:

```bash
# 1. Build the wheel (Rust + Python extension)
uv build --wheel

# 2. Reinstall into the active environment
pip install --force-reinstall --no-deps dist/pytracingx-*.whl

# 3. Run the test suite
pytest -v

# 4. Lint + format check (must pass for CI to accept)
ruff check python/pytracingx tests
ruff format --check python/pytracingx tests
```

Notes:
- The wheel build can take 1-3 minutes (Rust release compile + vendored OpenSSL).
- Tests rely on a session-scoped pytracingx runtime — they will all share one initialization.
- If `pytest` reports 40+ passing tests with no failures and ruff is clean, the change is verified.
- If you see `RuntimeError: pytracingx is already initialized` or `not initialized`, that's the one-shot dispatcher — investigate the test isolation logic in `tests/conftest.py` rather than retrying.
