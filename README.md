
# Rusqlite for Schemamama

A Rusqlite SQLite3 adapter for the lightweight database migration system
[Schemamama](https://github.com/SkylerLipthay/schemamama). Depends on the
`rusqlite` crate.

It is based on schemamama_postgres.

## Installation

Rusqlite requires sqlite3 dev library to be installed.
Don't forget you may need to pass in a custom value for the PKG_CONFIG_PATH
environment variable if it is not installed in usual the system-wide locations.

Then add Schemamama to your `Cargo.toml`:

```toml
[dependencies]
schemamama = "*"
schemamama_rusqlite = "*"
rusqlite = "0.2.0"
```

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
    fn up(&self, conn: &rusqlite::SqliteConnection) -> SqliteResult<()> {
        conn.execute("CREATE TABLE users (id BIGINT PRIMARY KEY);", &[]).map(|_| ())
    }

    fn down(&self, transaction: &postgres::Transaction) {
        transaction.execute("DROP TABLE users;", &[]).unwrap().map(|_| ())
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
let conn = SqliteConnection::open_in_memory().unwrap();
let adapter = SqliteAdapter::new(&conn);

// Create the metadata tables necessary for tracking migrations. This is safe to call more than
// once (`CREATE TABLE IF NOT EXISTS schemamama` is used internally):
adapter.setup_schema();

let mut migrator = Migrator::new(adapter);

migrator.register(Box::new(CreateUsers));
migrator.register(Box::new(CreateProducts));

// Execute migrations up to and including version 2:
migrator.up(2);
assert_eq!(migrator.current_version(), Some(1));

// Reverse all migrations:
migrator.down(None);
assert_eq!(migrator.current_version(), None);
```

## Testing

Run ```cargo test```

## To-do

* Make metadata table name configurable (currently locked in to `schemamama`).