//!##Example
//!
//!Setup the migrations:
//!
//!```
//!#[macro_use]
//!extern crate schemamama;
//!extern crate schemamama_rusqlite;
//!extern crate rusqlite;
//!
//!use schemamama::{Migration, Migrator};
//!use schemamama_rusqlite::{SqliteAdapter, SqliteMigration};
//!
//!struct CreateUsers;
//!// Instead of using sequential numbers (1, 2, 3...), you may instead choose to use a global
//!// versioning scheme, such as epoch timestamps.
//!migration!(CreateUsers, 1, "create users table");
//!
//!impl SqliteMigration for CreateUsers {
//!    fn up(&self, conn: &rusqlite::SqliteConnection) -> SqliteResult<()> {
//!        conn.execute("CREATE TABLE users (id BIGINT PRIMARY KEY);", &[]).map(|_| ())
//!    }
//!
//!    fn down(&self, transaction: &postgres::Transaction) {
//!        transaction.execute("DROP TABLE users;", &[]).unwrap().map(|_| ())
//!    }
//!}
//!
//!struct CreateProducts;
//!migration!(CreateProducts, 2, "create products table");
//!
//!impl SqliteMigration for CreateProducts {
//!    // ...
//!}
//!```
//!
//!Then, run the migrations!
//!
//!```
//!let conn = SqliteConnection::open_in_memory().unwrap();
//!let adapter = SqliteAdapter::new(&conn);
//!
//!// Create the metadata tables necessary for tracking migrations. This is safe to call more than
//!// once (`CREATE TABLE IF NOT EXISTS schemamama` is used internally):
//!adapter.setup_schema();
//!
//!let mut migrator = Migrator::new(adapter);
//!
//!migrator.register(Box::new(CreateUsers));
//!migrator.register(Box::new(CreateProducts));
//!
//!// Execute migrations up to and including version 2:
//!migrator.up(2);
//!assert_eq!(migrator.current_version(), Some(1));
//!
//!// Reverse all migrations:
//!migrator.down(None);
//!assert_eq!(migrator.current_version(), None);
//!```

#![doc(html_root_url = "https://cmsd2.github.io/rust-docs/schemamama_rusqlite/schemamama_rusqlite/")]

#[macro_use]
extern crate schemamama;
extern crate rusqlite;
#[macro_use]
extern crate log;

use schemamama::{Adapter, Migration, Version};
use std::collections::BTreeSet;
use rusqlite::{SqliteConnection,SqliteResult,SqliteStatement};

/// A migration to be used within a PostgreSQL connection.
pub trait SqliteMigration : Migration {
    /// Called when this migration is to be executed. This function has an empty body by default,
    /// so its implementation is optional.
    #[allow(unused_variables)]
    fn up(&self, conn: &SqliteConnection) -> SqliteResult<()> { Ok(()) }

    /// Called when this migration is to be reversed. This function has an empty body by default,
    /// so its implementation is optional.
    #[allow(unused_variables)]
    fn down(&self, conn: &SqliteConnection) -> SqliteResult<()> { Ok(()) }
}

/// An adapter that allows its migrations to act upon PostgreSQL connection transactions.
pub struct SqliteAdapter<'a> {
    connection: &'a SqliteConnection
}

impl<'a> SqliteAdapter<'a> {
    /// Create a new migrator tied to a PostgreSQL connection.
    pub fn new(connection: &'a SqliteConnection) -> SqliteAdapter {
        SqliteAdapter { connection: connection }
    }

    /// Create the tables Schemamama requires to keep track of schema state. If the tables already
    /// exist, this function has no operation.
    pub fn setup_schema(&self) {
        let query = "CREATE TABLE IF NOT EXISTS schemamama (version BIGINT PRIMARY KEY);";
        if let Err(e) = self.connection.execute(query, &[]) {
            panic!("Schema setup failed: {:?}", e);
        }
    }

    // Panics if `setup_schema` hasn't previously been called or if the insertion query otherwise
    // fails.
    fn record_version(&self, version: Version) -> SqliteResult<()> {
        let query = "INSERT INTO schemamama (version) VALUES ($1);";
        let mut stmt = self.connection.prepare(query).unwrap();
        
        match stmt.execute(&[&version]) {
            Err(e) => {
                warn!("Failed to delete version {:?}: {:?}", version, e);
                Err(e)
            }
            _ => Ok(())
        }
    }

    // Panics if `setup_schema` hasn't previously been called or if the deletion query otherwise
    // fails.
    fn erase_version(&self, version: Version) -> SqliteResult<()> {
        let query = "DELETE FROM schemamama WHERE version = $1;";
        let mut stmt = self.connection.prepare(query).unwrap();
        
        match stmt.execute(&[&version]) {
            Err(e) => {
                warn!("Failed to delete version {:?}: {:?}", version, e);
                Err(e)
            }
            _ => Ok(())
        }
    }

    fn execute_transaction<F>(&self, block: F) -> SqliteResult<()> where F: Fn(&SqliteConnection) -> SqliteResult<()> {
        let tx = try!(self.connection.transaction());
        
        try!(block(&self.connection));

        tx.commit()
    }

    fn prepare(&self, query: &str) -> SqliteStatement {
        match self.connection.prepare(query) {
            Ok(s) => s,
            Err(e) => panic!("Query preparation failed: {:?}", e)
        }
    }

}

impl<'a> Adapter for SqliteAdapter<'a> {
    type MigrationType = SqliteMigration;
    
    /// Panics if `setup_schema` hasn't previously been called or if the query otherwise fails.
    fn current_version(&self) -> Option<Version> {
        let query = "SELECT version FROM schemamama ORDER BY version DESC LIMIT 1;";

        let mut statement = self.prepare(query);

        let mut rows = statement.query(&[]).unwrap();

        rows.next().map(|row| row.unwrap().get(0) )
    }

    /// Panics if `setup_schema` hasn't previously been called or if the query otherwise fails.
    fn migrated_versions(&self) -> BTreeSet<Version> {
        let query = "SELECT version FROM schemamama;";

        let mut statement = self.prepare(query);

        let rows = statement.query(&[]).unwrap();

        rows.map(|v| v.unwrap().get(0) ).collect()
    }

    /// Panics if `setup_schema` hasn't previously been called or if the migration otherwise fails.
    fn apply_migration(&self, migration: &SqliteMigration) {
        self.execute_transaction(|transaction| {
            try!(migration.up(&transaction));
            try!(self.record_version(migration.version()));
            Ok(())
        }).unwrap();
    }

    /// Panics if `setup_schema` hasn't previously been called or if the migration otherwise fails.
    fn revert_migration(&self, migration: &SqliteMigration) {
        self.execute_transaction(|transaction| {
            try!(migration.down(&transaction));
            try!(self.erase_version(migration.version()));
            Ok(())
        }).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use schemamama::{Migrator};
    use rusqlite::{SqliteConnection,SqliteResult};

    
    struct CreateUsers;
    migration!(CreateUsers, 1, "create users table");

    impl SqliteMigration for CreateUsers {
        fn up(&self, conn: &SqliteConnection) -> SqliteResult<()> {
            conn.execute("CREATE TABLE users (id BIGINT PRIMARY KEY);", &[]).map(|_| ())
        }
        
        fn down(&self, conn: &SqliteConnection) -> SqliteResult<()> {
            conn.execute("DROP TABLE users;", &[]).map(|_| ())
        }
    }

    #[test]
    pub fn test_register() {
        let conn = SqliteConnection::open_in_memory().unwrap();

        let adapter = SqliteAdapter::new(&conn);

        adapter.setup_schema();

        let mut migrator = Migrator::new(adapter);

        migrator.register(Box::new(CreateUsers));

        migrator.up(1);

        assert_eq!(migrator.current_version(), Some(1));

        migrator.down(None);
        assert_eq!(migrator.current_version(), None);
    }
}
