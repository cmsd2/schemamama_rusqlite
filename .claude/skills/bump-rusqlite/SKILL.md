---
name: bump-rusqlite
description: Extend this crate's supported rusqlite range to cover a newer release - test the new version, raise the upper bound, move the CI matrix, and update the README. Use when asked to bump, upgrade or support a new rusqlite, to check whether a rusqlite version works, or when a new rusqlite has shipped.
---

# Extend the rusqlite range

Since 0.18 this crate depends on a **range** of rusqlite versions, not one:

```toml
rusqlite = ">=0.28, <0.41"
```

That range is the fix for [issue #6](https://github.com/cmsd2/schemamama_rusqlite/issues/6).
`libsqlite3-sys` is a `links = "sqlite3"` crate and Cargo permits one per graph, so a single
pinned rusqlite made this crate unusable alongside any project on a different rusqlite. Supporting
a range lets Cargo unify on the downstream project's choice.

**The job is now raising the upper bound, not moving a pin.** Do not narrow the range back to one
version. If a change seems to require that, stop and raise it with the user.

Ask whether this is a **trial** (find out whether a version works, commit nothing) or a
**release**. Default to a trial if they only asked whether a version works.

## 1. Find out whether the new version already works

Usually it does. The crate uses a small part of the rusqlite API and has gone whole spans of
releases untouched: 0.28 through 0.40 needed no source changes at all.

```bash
cargo search rusqlite --limit 1        # latest published
grep -n '^rusqlite' Cargo.toml         # the range we currently claim
```

Test the candidate against the current source before editing anything:

```bash
cargo update -p rusqlite --precise <version>   # only works inside the current range
cargo test
```

If the version is above the upper bound, `--precise` refuses it. Widen the bound in `Cargo.toml`
first, then run the above. Restore with `rm Cargo.lock && cargo update` if you are only trialling.

## 2. If it does not compile

The crate touches `Connection`, `Transaction`, `Statement::execute`, `query_row`, `query_map`,
`Row::get` and the `Error` enum. Breakage lands in parameter passing and error types. Past hops
needed `NO_PARAMS` replaced by `[]`, and a borrow dropped from `stmt.execute(&[&version])`.

If `Error` renames or removes variants, check `current_version` in the `Adapter` impl: it matches
`SqliteError::QueryReturnedNoRows` to return `Ok(None)`, so a rename there becomes a behaviour
change rather than a compile error.

Two outcomes worth separating:

- **A fix that compiles across the whole range.** Make it and keep the range whole.
- **A fix that only works above some version.** The range would have to be split with `cfg` or
  cut at the bottom. Stop and tell the user. Raising the lower bound drops support for projects
  on older rusqlite, which is the problem #6 was about, so it is their call and not a routine step.

Never change the signature of `SqliteMigration::up` or `down`. They take `&rusqlite::Connection`
and every downstream migration implements them. If rusqlite forces a change there, stop.

## 3. Raise the bound

In `Cargo.toml`, move the upper bound to just above the new version and raise the crate's own
`version` by one minor:

```toml
rusqlite = ">=0.28, <0.42"
```

The bound is exclusive, so supporting 0.41 means `<0.42`. The old one-to-one mapping between this
crate's minor and rusqlite's is dead; do not try to preserve it.

## 4. Move the CI matrix

`.github/workflows/ci.yml` has a `rusqlite-range` job pinning the two ends of the range. **Update
the top entry to the new version.** A range CI does not test is a claim, not a guarantee, and the
matrix drifting from `Cargo.toml` is the failure this job exists to prevent.

## 5. Update the README

The Compatability section states the supported range in prose, currently
"**0.18 works with any rusqlite from 0.28 to 0.40.**" Update the crate version and the top of the
range. The per-version table below it covers 0.10 to 0.17 only and is history; leave it alone.

Check the install example's `rusqlite = "0.40"` line and its comment too.

## 6. Verify

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
```

Then both ends of the new range, which is what CI will do:

```bash
for v in 0.28.0 <new-top>; do
  rm -f Cargo.lock && cargo update -q
  cargo update -p rusqlite --precise "$v" && cargo test
done
rm -f Cargo.lock && cargo update -q
```

And the Linux build against the system SQLite:

```bash
docker build -t schemamama_rusqlite .
```

Skip Docker only if the daemon is not running, and say so rather than implying it passed.

## 7. Release

Hand off to the `release-crate` skill. Do not publish from here.

## Reporting a trial

Leave the working tree dirty and report: the version tested, whether it compiled unchanged, what
broke if anything, and whether the fix holds across the whole range or would force the lower bound
up. Do not commit.
