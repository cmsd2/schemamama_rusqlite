# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this crate is

`schemamama_rusqlite` is a SQLite/rusqlite adapter for the
[Schemamama](https://github.com/SkylerLipthay/schemamama) migration system. It is a single-file
library (`src/lib.rs`, ~230 lines) ported from `schemamama_postgres`. Consumers write migrations,
Schemamama's `Migrator` drives them, and this crate supplies the database-specific half.

## Commands

```bash
cargo build
cargo test
cargo test test_register            # single test
cargo test -- --nocapture           # see log output (env_logger is a dev-dependency; set RUST_LOG)
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps
```

The last three are enforced by CI, so run them before pushing. `cargo clippy --fix` handles most
hits automatically.

Building needs the sqlite3 development library (`libsqlite3-dev` on Debian, `brew install sqlite`
on macOS). If linking fails, set `PKG_CONFIG_PATH` to the directory holding `sqlite3.pc`.

## Architecture

Two public items carry the whole design:

- **`SqliteMigration`** extends Schemamama's `Migration` with `up`/`down`, both taking
  `&rusqlite::Connection` and defaulting to no-ops. Users implement it alongside the
  `migration!(Type, version, description)` macro from the `schemamama` crate.
- **`SqliteAdapter`** implements Schemamama's `Adapter` with `MigrationType = dyn SqliteMigration`.
  It holds `Rc<RefCell<Connection>>` — the `RefCell` exists because `Connection::transaction()`
  needs `&mut Connection` while the read paths only need `&Connection`, and the `Rc` lets callers
  keep their own handle to the same connection.

Migration state lives in a table literally named `schemamama` with one `version BIGINT PRIMARY KEY`
column. `setup_schema()` creates it (`CREATE TABLE IF NOT EXISTS`, idempotent) and must be called
before any adapter method; every `Adapter` method assumes the table exists. Making the table name
configurable is the one open to-do in the README.

`apply_migration`/`revert_migration` wrap the user's `up`/`down` and the corresponding
version-row insert/delete in a single transaction via `execute_transaction`, so a failed
migration leaves no version recorded. The closure receives the `Transaction`, which derefs to
`Connection` — that is why migration bodies see a plain `&Connection`.

Errors: internal helpers return `rusqlite::Result`; the `Adapter` impl converts into
`SqliteMigrationError` (a `thiserror` enum wrapping `rusqlite::Error`). `setup_schema` panics
rather than returning an error, and `current_version` maps `QueryReturnedNoRows` to `Ok(None)`.

## Releasing

Version bumps track rusqlite releases one-for-one: this crate's only reason to release a new
version is to follow a new rusqlite. A release does three things in the same change:

1. Raise `version` in `Cargo.toml`.
2. Raise the `rusqlite` dependency in `Cargo.toml`.
3. **Add a row to the compatibility table in `README.md`** mapping the new crate version to its
   rusqlite and libsqlite3-sys versions.

Step 3 is not optional and is easy to forget. That table is how users pick a version of this crate,
and a stale or missing row makes the release unusable to them. Find the libsqlite3-sys version by
checking what the new rusqlite pins (`cargo tree -p libsqlite3-sys` after the bump, or rusqlite's
own `Cargo.toml`) rather than guessing from the pattern of previous rows. Verify the table matches
`Cargo.toml` before committing, and never bump `Cargo.toml` in one commit and the README in another.

## CI

`.github/workflows/ci.yml` runs on push to `master`, on pull requests, and on manual dispatch. Two
jobs, both on `ubuntu-latest`:

- `test` — matrix over stable and beta, `cargo build` then `cargo test`.
- `lint` — stable only: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo doc --no-deps`.

Both install `libsqlite3-dev` via apt before building, since rusqlite links against the system
SQLite here (the `bundled` feature is not enabled). `RUSTFLAGS: -D warnings` is set workflow-wide,
so a plain compiler warning fails the build too.

Docs are not built or uploaded by CI beyond the `cargo doc` check — docs.rs publishes them
automatically on each crates.io release.

## Notes

- Several doc comments still say "PostgreSQL" from the port. Fix them opportunistically.
- There is no `tests/` directory; the only test is the `mod tests` block at the bottom of
  `src/lib.rs`, using an in-memory connection. Note `Cargo.toml` spells the section
  `[dev_dependencies]` (underscore) — Cargo still honours it, but the canonical spelling is
  `dev-dependencies`.
- The root `Dockerfile` is a local stand-in for CI, not a shipped artifact. `docker build .` runs
  fmt, clippy, build, test and doc in a clean Debian environment; `docker run --rm -it` on the
  built image drops into a shell there. It tracks `ci.yml` by hand — change both together. The
  official `rust` image ships stable only, so the Docker path cannot cover CI's beta job.
