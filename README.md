# Rusqlite for Schemamama

A Rusqlite SQLite3 adapter for the lightweight database migration system
[Schemamama](https://github.com/SkylerLipthay/schemamama). Depends on the
`rusqlite` crate.

It is based on schemamama_postgres.

## Installation

Rusqlite requires sqlite3 dev library to be installed.

Then add Schemamama to your `Cargo.toml`:

```toml
[dependencies]
schemamama = "*"
schemamama_rusqlite = "*"
rusqlite = "0.2.0"
```

You may need to pass in a custom value for the PKG_CONFIG_PATH if rust is unable
to locate your sqlite3 installation.

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

Run `cargo test`

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
