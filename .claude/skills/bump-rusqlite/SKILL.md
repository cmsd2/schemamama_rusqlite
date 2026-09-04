---
name: bump-rusqlite
description: Upgrade this crate to a newer rusqlite release - retarget the dependency, fix whatever the new API breaks, update the README compatibility table, and verify. Use when asked to bump, upgrade or update rusqlite, to check whether a new rusqlite works, to cut a release, or when a new rusqlite version has shipped.
---

# Bump rusqlite

This crate exists to track rusqlite. A new rusqlite minor is the only reason it releases,
and each release is one commit that moves the dependency, the crate version and the README
compatibility table together.

Ask whether the user wants a **trial** (find out what breaks, leave nothing committed) or a
**release** (commit, and optionally publish). Default to a trial if they only asked whether a
version works.

## 1. Pick the target

```bash
cargo search rusqlite --limit 1        # latest published
grep -n '^rusqlite' Cargo.toml         # where we are now
```

rusqlite breaks its API most minors. **If the crate is more than one minor behind, go one minor
at a time** unless told otherwise: each hop is a small, comprehensible diff, and a compile error
after eight hops tells you nothing about which hop caused it. Every intermediate hop still gets
its own README row, because users pin to those versions.

Get the version to bump to, then check what actually changed before editing anything:

```bash
gh api repos/rusqlite/rusqlite/releases --jq '.[] | "\(.tag_name)\t\(.published_at[:10])"' | head -20
gh api repos/rusqlite/rusqlite/releases/tags/v<target> --jq .body
```

## 2. Move the dependency

Edit `Cargo.toml`: set `rusqlite` to the target, and raise the crate's own `version` by one
minor. The mapping is one-to-one and has been since 0.10 — rusqlite 0.32 is this crate 0.17,
rusqlite 0.33 is 0.18, and so on. Never skip a crate minor even when skipping rusqlite minors.

## 3. Fix the breakage

```bash
cargo build 2>&1 | head -40
```

`src/lib.rs` is the only source file, about 230 lines, and touches a narrow part of rusqlite:
`Connection`, `Transaction`, `Statement::execute`, `query_row`, `query_map`, `Row::get`, and the
`Error` enum. Breakage lands in the parameter-passing and error types. Past hops needed:

- `NO_PARAMS` deprecated in favour of `[]` (commit a6be765).
- `stmt.execute(&[&version])` losing its borrow, becoming `stmt.execute([&version])`.

If `Error` gains or renames variants, check `current_version` in the `Adapter` impl — it matches
`SqliteError::QueryReturnedNoRows` by name to return `Ok(None)`, and a rename there compiles
into a behaviour change elsewhere rather than an error.

Keep the public API stable if you possibly can. `SqliteMigration::up`/`down` take
`&rusqlite::Connection`, so a signature change there breaks every downstream migration. If a
rusqlite release forces one, stop and tell the user before proceeding — that is a bigger
decision than a version bump.

## 4. Update the compatibility table

**This is the step that gets forgotten, and the one users depend on most.** The table in
`README.md` maps each version of this crate to its rusqlite and libsqlite3-sys versions; a
release without its row is unusable to anyone choosing a version.

Read the libsqlite3-sys version off the resolved tree rather than guessing from the pattern —
rusqlite does not raise it every minor:

```bash
cargo tree -p libsqlite3-sys | head -1
```

Add one row per crate minor, in order. Record the minor only (`0.30`, not `0.30.1`), matching
the existing rows.

## 5. Verify

Everything CI enforces, in order:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
```

`cargo clippy --fix` handles most lint hits; a new rusqlite often trips new ones. Then confirm
it builds on Linux against the system SQLite, which is what CI and most users get:

```bash
docker build -t schemamama_rusqlite .
```

The Docker build runs the same checks, so a successful build is the whole gate. Skip it only if
the daemon is not running, and say so rather than implying it passed.

Check the README's usage example still compiles as written if the API changed — it is not a
doctest, so nothing catches it drifting.

## 6. Commit and release

One commit per hop, containing `Cargo.toml`, `README.md` and any `src/lib.rs` fixes together.
Never split the manifest and the table across commits. The existing history uses
`bump versions` as the subject; keep it, and add a body when the hop needed source changes.

Publishing is the user's call — ask, do not assume:

```bash
cargo publish --dry-run
cargo publish
```

`cargo publish` is irreversible: a version number on crates.io can be yanked but never reused.
Confirm explicitly before running it. After publishing, docs.rs builds the documentation on its
own; there is nothing to upload.

## Reporting a trial

If this was a trial, leave the working tree dirty and report: the target version, what broke,
what the fix would be, whether the public API survives unchanged, and the libsqlite3-sys version
the new row would name. Do not commit.
