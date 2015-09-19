#[macro_use]
extern crate schemamama;
extern crate rusqlite;

use schemamama::{Adapter, Migration, Version};
use std::collections::BTreeSet;
use rusqlite::{SqliteConnection,SqliteResult,SqliteStatement};

/// A migration to be used within a PostgreSQL connection.
pub trait Sqlite3Migration : Migration {
    /// Called when this migration is to be executed. This function has an empty body by default,
    /// so its implementation is optional.
    #[allow(unused_variables)]
    fn up(&self, conn: &SqliteConnection) { }

    /// Called when this migration is to be reversed. This function has an empty body by default,
    /// so its implementation is optional.
    #[allow(unused_variables)]
    fn down(&self, conn: &SqliteConnection) { }
}

/// An adapter that allows its migrations to act upon PostgreSQL connection transactions.
pub struct Sqlite3Adapter<'a> {
    connection: &'a SqliteConnection
}

impl<'a> Sqlite3Adapter<'a> {
    /// Create a new migrator tied to a PostgreSQL connection.
    pub fn new(connection: &'a SqliteConnection) -> Sqlite3Adapter {
        Sqlite3Adapter { connection: connection }
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
    fn record_version(&self, version: Version) {
        let query = "INSERT INTO schemamama (version) VALUES ($1);";
        let mut stmt = self.connection.prepare(query).unwrap();
        
        if let Err(e) = stmt.execute(&[&version]) {
            panic!("Failed to delete version {:?}: {:?}", version, e);
        }
    }

    // Panics if `setup_schema` hasn't previously been called or if the deletion query otherwise
    // fails.
    fn erase_version(&self, version: Version) {
        let query = "DELETE FROM schemamama WHERE version = $1;";
        let mut stmt = self.connection.prepare(query).unwrap();
        
        if let Err(e) = stmt.execute(&[&version]) {
            panic!("Failed to delete version {:?}: {:?}", version, e);
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

impl<'a> Adapter for Sqlite3Adapter<'a> {
    type MigrationType = Sqlite3Migration;
    
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
    fn apply_migration(&self, migration: &Sqlite3Migration) {
        self.execute_transaction(|transaction| {
            migration.up(&transaction);
            self.record_version(migration.version());
            Ok(())
        }).unwrap();
    }

    /// Panics if `setup_schema` hasn't previously been called or if the migration otherwise fails.
    fn revert_migration(&self, migration: &Sqlite3Migration) {
        self.execute_transaction(|transaction| {
            migration.down(&transaction);
            self.erase_version(migration.version());
            Ok(())
        }).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use schemamama::{Migrator};
    use rusqlite::SqliteConnection;

    
    struct CreateUsers;
    migration!(CreateUsers, 1, "create users table");

    impl Sqlite3Migration for CreateUsers {
        fn up(&self, conn: &SqliteConnection) {
            conn.execute("CREATE TABLE users (id BIGINT PRIMARY KEY);", &[]).unwrap();
        }
        
        fn down(&self, conn: &SqliteConnection) {
            conn.execute("DROP TABLE users;", &[]).unwrap();
        }
    }

    #[test]
    pub fn test_register() {
        let conn = SqliteConnection::open_in_memory().unwrap();

        let adapter = Sqlite3Adapter::new(&conn);

        adapter.setup_schema();

        let mut migrator = Migrator::new(adapter);

        migrator.register(Box::new(CreateUsers));

        migrator.up(1);

        assert_eq!(migrator.current_version(), Some(1));

        migrator.down(None);
        assert_eq!(migrator.current_version(), None);
    }
}
