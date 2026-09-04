---
name: release-crate
description: Publish a new version of this crate to crates.io - verify, bump the version, commit, wait for CI, publish, tag and push. Use when asked to release, publish, cut a version, ship to crates.io, or tag a release.
---

# Release to crates.io

Publishing is irreversible. A version number can be yanked but never reused, and a yanked
version stays downloadable for anyone who already pinned it. **Run every check first, then stop
and ask before `cargo publish` and before pushing the tag.** Those two commands are the user's
call, always, even mid-flow and even if they asked for a release.

For a rusqlite version bump, use the `bump-rusqlite` skill first — it produces the commit this
skill then releases.

## 1. Preconditions

```bash
git status --short              # must be clean
git branch --show-current       # master
git fetch origin && git log --oneline master..origin/master   # must be empty
cargo owner --list              # confirms publish rights and that you are logged in
```

A dirty tree is the common blocker. `cargo publish` refuses one unless forced, and forcing it
publishes files that are not in git — never pass `--allow-dirty` here.

If `cargo owner` fails on authentication, the user needs `cargo login` with a crates.io token.
That is theirs to run; do not ask for the token or handle it.

## 2. Verify

Everything CI enforces, plus the Linux build:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
docker build -t schemamama_rusqlite .
```

Do not proceed on a failure, and do not describe a skipped check as passing. If Docker is not
running, say so.

## 3. Choose the version

Edit `version` in `Cargo.toml`. This crate is pre-1.0, so a breaking change is a minor bump and
everything else is a patch. In practice releases exist to follow rusqlite, and those are always
minor bumps with a fixed mapping — see `bump-rusqlite`.

Confirm the version is not already taken; the dry run warns rather than errors on this, so it is
easy to miss:

```bash
cargo search schemamama_rusqlite --limit 1
```

**If the release follows a rusqlite bump, the `README.md` compatibility table must already have
its row.** Check before going further. A published version missing from that table is the one
release mistake users actually notice.

## 4. Check what ships

```bash
cargo package --list
```

Read the list. The package is built from git-tracked files, so `.gitignore` controls it but
`.dockerignore` does not, and development files end up shipped unless `exclude` in `Cargo.toml`
says otherwise. Anything under `.github/`, `.claude/`, `CLAUDE.md`, `Dockerfile` is dead weight
in the published tarball. Confirm `README.md`, both licence files and `src/` are present.

Then the full rehearsal, which packages and compiles from the packaged tarball:

```bash
cargo publish --dry-run
```

## 5. Commit and let CI confirm

```bash
git commit -am "bump versions"      # existing history uses this subject
git push origin master
gh run watch "$(gh run list --limit 1 --json databaseId -q '.[0].databaseId')" --exit-status
```

`gh run watch` needs an explicit run id when not attached to a terminal, and the run takes a few
seconds to register after the push — if the id comes back as the previous run, wait and re-read
it rather than watching the wrong one.

Publish from a commit CI has gone green on, not from a local tree.

## 6. Publish — ask first

Stop here. Report what will be published: crate name, version, the rusqlite version it targets,
and anything notable in the file list. Get an explicit yes.

```bash
cargo publish
```

## 7. Tag

Existing history uses lightweight `vX.Y.Z` tags on the version-bump commit, pushed to origin:

```bash
git tag v0.18.0
git push origin v0.18.0
```

Match that. No annotated tags, no `release-` prefix.

## 8. After

docs.rs builds the documentation itself within a few minutes; there is nothing to upload, and no
GitHub Release is created for this repo. Confirm the release landed:

```bash
cargo search schemamama_rusqlite --limit 1
```

If something is wrong with a published version, `cargo yank --version X.Y.Z` stops new
dependents resolving to it. Yanking does not delete it and does not break existing users. The
fix for a bad release is always a new version, never a reused one.
