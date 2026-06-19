---
name: bump-version
description: Bump the project version across pyproject.toml, Cargo.toml, and Cargo.lock, then optionally commit the change and create a local git tag. NEVER pushes to remote — the user is responsible for git push. Use when the user asks to bump to a new version.
disable-model-invocation: true
---

Argument: `$ARGUMENTS` — the new version (e.g., `0.1.3`).

Steps:

1. Validate the argument looks like semver: `MAJOR.MINOR.PATCH` (no `v` prefix).
2. Update `pyproject.toml`: change the line `version = "OLD"` under `[project]` to `version = "NEW"`.
3. Update `Cargo.toml`: change the line `version = "OLD"` under `[package]` to `version = "NEW"`.
4. Update `Cargo.lock` so the entry for `name = "pytracingx"` matches:
   ```bash
   cargo update -p pytracingx --precise $ARGUMENTS
   ```
   (Note: `Cargo.lock` is gitignored; this keeps local builds consistent but the change won't be committed.)
5. Show the user the diff of `pyproject.toml` and `Cargo.toml`.
6. Ask the user whether to also commit and tag locally. If yes, run:
   ```bash
   git add pyproject.toml Cargo.toml
   git commit -m "chore: bump to $ARGUMENTS"
   git tag v$ARGUMENTS
   ```
7. **Never push.** Do not run `git push`, `git push --tags`, or `git push origin v$ARGUMENTS` under any circumstances. The user will push manually when they're ready to trigger the release workflow:
   ```bash
   git push && git push origin v$ARGUMENTS
   ```

If the new version equals the existing version, abort and tell the user nothing changed.
