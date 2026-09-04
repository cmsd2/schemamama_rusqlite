# Rusqlite for Schemamama

[![CI](https://github.com/cmsd2/schemamama_rusqlite/actions/workflows/ci.yml/badge.svg)](https://github.com/cmsd2/schemamama_rusqlite/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/schemamama_rusqlite.svg)](https://crates.io/crates/schemamama_rusqlite)
[![docs.rs](https://docs.rs/schemamama_rusqlite/badge.svg)](https://docs.rs/schemamama_rusqlite)

A Rusqlite SQLite3 adapter for the lightweight database migration system
[Schemamama](https://github.com/SkylerLipthay/schemamama). Depends on the
`rusqlite` crate.

It is based on schemamama_postgres.

## Installation

Rusqlite requires sqlite3 dev library to be installed.

Then add Schemamama to your `Cargo.toml`:

```toml
[dependencies]
schemamama = "0.3"
schemamama_rusqlite = "0.18"
rusqlite = "0.40"   # any version from 0.28 to 0.40
```

You may need to pass in a custom value for the PKG_CONFIG_PATH if rust is unable
to locate your sqlite3 installation.

## Compatability

**0.18 works with any rusqlite from 0.28 to 0.40.**

From 0.18 this package accepts a range rather than a single version, so you choose the
rusqlite your project needs and this package follows. Whichever `libsqlite3-sys` your
rusqlite pulls in is the one you get. CI tests both ends of the range on every commit.

### Why the range matters

`libsqlite3-sys` is a `links = "sqlite3"` crate, and Cargo allows only one such
package in a dependency graph. Earlier versions of this package pinned a single
rusqlite minor, so any project wanting a different one hit:

```
error: multiple packages link to native library `sqlite3`,
       but a native library can be linked only once
```

That was [issue #6](https://github.com/cmsd2/schemamama_rusqlite/issues/6), and the
range fixes it. Cargo resolves your rusqlite requirement and this package's range to
a single version, so there is one `libsqlite3-sys` and no conflict.

If you need a rusqlite newer than the top of the range, open an issue. Widening the
range is usually a one-line change, because this package uses a small and stable part
of the rusqlite API.

### Older versions

Versions before 0.18 pin exactly one rusqlite minor, and are subject to the conflict
described above:

|This package|Rusqlite|libsqlite3-sys|
|------------|--------|--------------|
|0.10        |0.25    |0.22          |
|0.11        |0.26    |0.23          |
|0.12        |0.27    |0.24          |
|0.13        |0.28    |0.25          |
|0.14        |0.29    |0.26          |
|0.15        |0.30    |0.27          |
|0.16        |0.31    |0.28          |
|0.17        |0.32    |0.30          |

## Usage

First, define some migrations:

```rust
#[macro_use]
extern crate schemamama;
extern crate schemamama_rusqlite;
extern crate rusqlite;

use schemamama::{Migration, Migrator};
use schemamama_rusqlite::{SqliteAdapter, SqliteMigration};

struct CreateUsers;
// Instead of using sequential numbers (1, 2, 3...), you may instead choose to use a global
// versioning scheme, such as epoch timestamps.
migration!(CreateUsers, 1, "create users table");

impl SqliteMigration for CreateUsers {
    fn up(&self, conn: &rusqlite::Connection) -> SqliteResult<()> {
        conn.execute("CREATE TABLE users (id BIGINT PRIMARY KEY);", []).map(|_| ())
    }

    fn down(&self, transaction: &rusqlite::Connection) -> SqliteResult<()> {
        transaction.execute("DROP TABLE users;", []).map(|_| ())
    }
}

struct CreateProducts;
migration!(CreateProducts, 2, "create products table");

impl SqliteMigration for CreateProducts {
    // ...
}
```

Then, run the migrations!

```rust
let conn = Rc::new(RefCell::new(SqliteConnection::open_in_memory().expect("open db")));
let adapter = SqliteAdapter::new(conn);

// Create the metadata tables necessary for tracking migrations. This is safe to call more than
// once (`CREATE TABLE IF NOT EXISTS schemamama` is used internally):
adapter.setup_schema();

let mut migrator = Migrator::new(adapter);

migrator.register(Box::new(CreateUsers));
migrator.register(Box::new(CreateProducts));

// Execute migrations up to and including version 2:
migrator.up(Some(2));
assert_eq!(migrator.current_version().expect("current version"), Some(2));

// Reverse all migrations:
migrator.down(None);
assert_eq!(migrator.current_version().expect("current version"), None);
```

## Testing

Run `cargo test`.

CI runs on GitHub Actions (`.github/workflows/ci.yml`): build and test on stable and beta, plus
`cargo fmt --check`, `cargo clippy -D warnings` and `cargo doc` on stable. Documentation is
published by [docs.rs](https://docs.rs/schemamama_rusqlite) on each crates.io release.

To run the same checks locally in a clean Linux environment, without installing sqlite3 headers on
your machine:

```
docker build -t schemamama_rusqlite .
```

The build fails if any check fails. `docker run --rm -it schemamama_rusqlite` gives you a shell in
that environment.

## To-do

- Make metadata table name configurable (currently locked in to `schemamama`).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE-2.0](LICENSE-APACHE-2.0) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
  at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you shall be dual licensed as above, without any
additional terms or conditions.
