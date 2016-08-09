
#![doc(html_root_url = "https://cmsd2.github.io/rust-docs/schemamama_rusqlite/schemamama_rusqlite/")]

#[macro_use]
extern crate schemamama;
extern crate rusqlite;
#[macro_use]
extern crate log;

use schemamama::{Adapter, Migration, Version};
use std::collections::BTreeSet;
use rusqlite::{SqliteConnection,SqliteResult,SqliteStatement};

#[derive(Debug)]
pub enum SqliteMigrationError {
    UknownError,
    RusqliteError(rusqlite::Error),
    SqlError(String),
}

impl From<rusqlite::Error> for SqliteMigrationError {
    fn from(err: rusqlite::Error) -> SqliteMigrationError {
        SqliteMigrationError::RusqliteError(err)
    }
}

pub type Result<T> = std::result::Result<T, SqliteMigrationError>;

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

impl <'a> SqliteAdapter<'a> {
    /// Create a new migrator tied to a SQLite connection.
    pub fn new(connection: &'a SqliteConnection) -> SqliteAdapter<'a> {
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
        let mut stmt = try!(self.connection.prepare(query));
        
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
        
        try!(block(self.connection));

        tx.commit()
    }

    fn prepare(&self, query: &str) -> Result<SqliteStatement> {
        self.connection.prepare(query).map_err(SqliteMigrationError::from)
    }

}

impl <'a> Adapter for SqliteAdapter<'a> {
    type MigrationType = SqliteMigration;

    type Error = SqliteMigrationError;
    
    /// Panics if `setup_schema` hasn't previously been called or if the query otherwise fails.
    fn current_version(&self) -> Result<Option<Version>> {
        let query = "SELECT version FROM schemamama ORDER BY version DESC LIMIT 1;";

        let mut statement = try!(self.prepare(query));
        let mut rows = try!(statement.query(&[]));

        if let Some(row_result) = rows.next() {
            let val = try!(row_result).get(0);
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    /// Panics if `setup_schema` hasn't previously been called or if the query otherwise fails.
    fn migrated_versions(&self) -> Result<BTreeSet<Version>> {
        let query = "SELECT version FROM schemamama;";

        let mut statement = try!(self.prepare(query));

        let rows = try!(statement.query_map(&[], |row_result| {
            row_result.get(0)
        }));

        let mut versions = BTreeSet::new();

        for vresult in rows {
            versions.insert(try!(vresult));
        }

        Ok(versions)
    }

    /// Panics if `setup_schema` hasn't previously been called or if the migration otherwise fails.
    fn apply_migration(&self, migration: &SqliteMigration) -> Result<()> {
        try!(self.execute_transaction(|transaction| {
            try!(migration.up(&transaction));
            try!(self.record_version(migration.version()));
            Ok(())
        }));

        Ok(())
    }

    /// Panics if `setup_schema` hasn't previously been called or if the migration otherwise fails.
    fn revert_migration(&self, migration: &SqliteMigration) -> Result<()> {
        try!(self.execute_transaction(|transaction| {
            try!(migration.down(&transaction));
            try!(self.erase_version(migration.version()));
            Ok(())
        }));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{SqliteMigration,SqliteAdapter};

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

        migrator.up(Some(1)).unwrap();

        assert_eq!(migrator.current_version().unwrap(), Some(1));

        migrator.down(None).unwrap();
        
        assert_eq!(migrator.current_version().unwrap(), None);
    }
}
