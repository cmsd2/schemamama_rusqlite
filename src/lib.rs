
#![doc(html_root_url = "https://cmsd2.github.io/rust-docs/schemamama_rusqlite/schemamama_rusqlite/")]

#[macro_use]
extern crate schemamama;
extern crate rusqlite;
#[macro_use]
extern crate log;

use schemamama::{Adapter, Migration, Version};
use std::collections::BTreeSet;
use rusqlite::{Connection,Statement,NO_PARAMS, Transaction};

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
    fn up(&self, conn: &Transaction) -> rusqlite::Result<()> { Ok(()) }

    /// Called when this migration is to be reversed. This function has an empty body by default,
    /// so its implementation is optional.
    #[allow(unused_variables)]
    fn down(&self, conn: &Transaction) -> rusqlite::Result<()> { Ok(()) }
}

/// An adapter that allows its migrations to act upon PostgreSQL connection transactions.
pub struct SqliteAdapter<'a> {
    connection: &'a mut Connection
}

impl <'a> SqliteAdapter<'a> {
    /// Create a new migrator tied to a SQLite connection.
    pub fn new(connection: &'a mut Connection) -> SqliteAdapter<'a> {
        SqliteAdapter { connection }
    }

    /// Create the tables Schemamama requires to keep track of schema state. If the tables already
    /// exist, this function has no operation.
    pub fn setup_schema(&self) {
        let query = "CREATE TABLE IF NOT EXISTS schemamama (version BIGINT PRIMARY KEY);";
        if let Err(e) = self.connection.execute(query, NO_PARAMS) {
            panic!("Schema setup failed: {:?}", e);
        }
    }

    // Panics if `setup_schema` hasn't previously been called or if the insertion query otherwise
    // fails.
    fn record_version(transaction: &Transaction, version: Version) -> rusqlite::Result<()> {
        let query = "INSERT INTO schemamama (version) VALUES ($1);";
        let mut stmt = transaction.prepare(query)?;
        
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
    fn erase_version(transaction: &Transaction, version: Version) -> rusqlite::Result<()> {
        let query = "DELETE FROM schemamama WHERE version = $1;";
        let mut stmt = transaction.prepare(query).unwrap();
        
        match stmt.execute(&[&version]) {
            Err(e) => {
                warn!("Failed to delete version {:?}: {:?}", version, e);
                Err(e)
            }
            _ => Ok(())
        }
    }

    fn execute_transaction<F>(&mut self, block: F) -> rusqlite::Result<()> where F: Fn(& Transaction) -> rusqlite::Result<()> {
        let tx = self.connection.transaction()?;
        
        block(&tx)?;

        tx.commit()
    }

    fn prepare(&self, query: &str) -> Result<Statement> {
        self.connection.prepare(query).map_err(SqliteMigrationError::from)
    }

}

impl <'a> Adapter for SqliteAdapter<'a> {
    type MigrationType = dyn SqliteMigration;

    type Error = SqliteMigrationError;
    
    /// Panics if `setup_schema` hasn't previously been called or if the query otherwise fails.
    fn current_version(&mut self) -> Result<Option<Version>> {
        let query = "SELECT version FROM schemamama ORDER BY version DESC LIMIT 1;";

        let mut statement = self.prepare(query)?;
        let mut rows = statement.query(NO_PARAMS)?;

        let next = rows.next();
        if let Ok(row_result) = next {
            if let Some(val) = row_result {
                Ok(Some(val.get(0)?))
            } else {
                // No rows exist
                Ok(None)
            }
        } else {
            next.map(|_|None).map_err(SqliteMigrationError::from)
        }
    }

    /// Panics if `setup_schema` hasn't previously been called or if the query otherwise fails.
    fn migrated_versions(&mut self) -> Result<BTreeSet<Version>> {
        let query = "SELECT version FROM schemamama;";

        let mut statement = self.prepare(query)?;

        let rows = statement.query_map(NO_PARAMS, |row_result| {
            row_result.get(0)
        })?;

        let mut versions = BTreeSet::new();

        for vresult in rows {
            versions.insert(vresult?);
        }

        Ok(versions)
    }

    /// Panics if `setup_schema` hasn't previously been called or if the migration otherwise fails.
    fn apply_migration(&mut self, migration: &dyn SqliteMigration) -> Result<()> {
        self.execute_transaction(|transaction| {
            migration.up(&transaction)?;
            SqliteAdapter::record_version(&transaction, migration.version())?;
            Ok(())
        })?;

        Ok(())
    }

    /// Panics if `setup_schema` hasn't previously been called or if the migration otherwise fails.
    fn revert_migration(&mut self, migration: &dyn SqliteMigration) -> Result<()> {
        self.execute_transaction(|transaction| {
            migration.down(&transaction)?;
            SqliteAdapter::erase_version(&transaction, migration.version())?;
            Ok(())
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{SqliteMigration,SqliteAdapter};

    use schemamama::{Migrator};
    use rusqlite::{Connection, NO_PARAMS, Transaction};

    
    struct CreateUsers;
    migration!(CreateUsers, 1, "create users table");

    impl SqliteMigration for CreateUsers {
        fn up(&self, conn: &Transaction) -> rusqlite::Result<()> {
            conn.execute("CREATE TABLE users (id BIGINT PRIMARY KEY);", NO_PARAMS).map(|_| ())
        }
        
        fn down(&self, conn: &Transaction) -> rusqlite::Result<()> {
            conn.execute("DROP TABLE users;", NO_PARAMS).map(|_| ())
        }
    }

    #[test]
    pub fn test_register() {
        let mut conn = Connection::open_in_memory().unwrap();

        let adapter = SqliteAdapter::new(&mut conn);

        adapter.setup_schema();

        let mut migrator = Migrator::new(adapter);

        migrator.register(Box::new(CreateUsers));

        migrator.up(Some(1)).unwrap();

        assert_eq!(migrator.current_version().unwrap(), Some(1));

        migrator.down(None).unwrap();
        
        assert_eq!(migrator.current_version().unwrap(), None);
    }
}
