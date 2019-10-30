
#![doc(html_root_url = "https://cmsd2.github.io/rust-docs/schemamama_rusqlite/schemamama_rusqlite/")]

#[allow(unused_imports)]
#[macro_use]
extern crate schemamama;
extern crate rusqlite;
#[macro_use]
extern crate log;

use schemamama::{Adapter, Migration, Version};
use std::collections::BTreeSet;
use rusqlite::{
    NO_PARAMS,
    Connection as SqliteConnection,
    Result as SqliteResult,
    Row as SqliteRow,
    Error as SqliteError,
};
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Debug)]
pub enum SqliteMigrationError {
    UknownError,
    RusqliteError(SqliteError),
    SqlError(String),
}

impl From<SqliteError> for SqliteMigrationError {
    fn from(err: SqliteError) -> SqliteMigrationError {
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
pub struct SqliteAdapter {
    connection: Rc<RefCell<SqliteConnection>>
}

impl SqliteAdapter {
    /// Create a new migrator tied to a SQLite connection.
    pub fn new(connection: Rc<RefCell<SqliteConnection>>) -> SqliteAdapter {
        SqliteAdapter { connection: connection }
    }

    /// Create the tables Schemamama requires to keep track of schema state. If the tables already
    /// exist, this function has no operation.
    pub fn setup_schema(&self) {
        let conn = self.connection.borrow();

        let query = "CREATE TABLE IF NOT EXISTS schemamama (version BIGINT PRIMARY KEY);";
        if let Err(e) = conn.execute(query, NO_PARAMS) {
            panic!("Schema setup failed: {:?}", e);
        }
    }

    // Panics if `setup_schema` hasn't previously been called or if the insertion query otherwise
    // fails.
    fn record_version(&self, conn: &SqliteConnection, version: Version) -> SqliteResult<()> {
        let query = "INSERT INTO schemamama (version) VALUES ($1);";
        let mut stmt = conn.prepare(query)?;
        
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
    fn erase_version(&self, conn: &SqliteConnection, version: Version) -> SqliteResult<()> {
        let query = "DELETE FROM schemamama WHERE version = $1;";
        let mut stmt = conn.prepare(query).unwrap();
        
        match stmt.execute(&[&version]) {
            Err(e) => {
                warn!("Failed to delete version {:?}: {:?}", version, e);
                Err(e)
            }
            _ => Ok(())
        }
    }

    fn execute_transaction<F>(&self, block: F) -> SqliteResult<()> where F: Fn(&SqliteConnection) -> SqliteResult<()> {
        let mut conn = self.connection.borrow_mut();

        let tx = conn.transaction()?;
        
        block(&tx)?;

        tx.commit()
    }

    fn query_row<T, F>(&self, q: &str, block: F) -> SqliteResult<T> where F: FnOnce(&SqliteRow) -> SqliteResult<T> {
        let conn = self.connection.borrow();

        let result = conn.query_row(q, NO_PARAMS, block)?;

        Ok(result)
    }

    fn query_map<T, F>(&self, q: &str, block: F) -> SqliteResult<Vec<T>> where F: FnMut(&SqliteRow) -> SqliteResult<T> {
        let conn = self.connection.borrow();

        let mut statement = conn.prepare(q)?;

        let result = statement.query_map(NO_PARAMS, block)?;

        result.collect()
    }
}

impl Adapter for SqliteAdapter {
    type MigrationType = dyn SqliteMigration;

    type Error = SqliteMigrationError;
    
    /// Panics if `setup_schema` hasn't previously been called or if the query otherwise fails.
    fn current_version(&self) -> Result<Option<Version>> {
        let query = "SELECT version FROM schemamama ORDER BY version DESC LIMIT 1;";

        match self.query_row(query, |row| row.get(0)) {
            Ok(version) => Ok(Some(version)),
            Err(SqliteError::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into())
        }
    }

    /// Panics if `setup_schema` hasn't previously been called or if the query otherwise fails.
    fn migrated_versions(&self) -> Result<BTreeSet<Version>> {
        let query = "SELECT version FROM schemamama;";

        let rows = self.query_map(query, |row_result| {
            row_result.get::<usize, i64>(0)
        })?;

        let mut versions = BTreeSet::new();

        for vresult in rows {
            versions.insert(vresult);
        }

        Ok(versions)
    }

    /// Panics if `setup_schema` hasn't previously been called or if the migration otherwise fails.
    fn apply_migration(&self, migration: &dyn SqliteMigration) -> Result<()> {
        self.execute_transaction(|transaction| {
            migration.up(&transaction)?;
            self.record_version(transaction, migration.version())?;
            Ok(())
        })?;

        Ok(())
    }

    /// Panics if `setup_schema` hasn't previously been called or if the migration otherwise fails.
    fn revert_migration(&self, migration: &dyn SqliteMigration) -> Result<()> {
        self.execute_transaction(|transaction| {
            migration.down(&transaction)?;
            self.erase_version(transaction, migration.version())?;
            Ok(())
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{SqliteMigration,SqliteAdapter};

    use schemamama::{Migrator};
    use rusqlite::{NO_PARAMS, Connection as SqliteConnection,Result as SqliteResult};
    use std::rc::Rc;
    use std::cell::RefCell;
    
    struct CreateUsers;
    migration!(CreateUsers, 1, "create users table");

    impl SqliteMigration for CreateUsers {
        fn up(&self, conn: &SqliteConnection) -> SqliteResult<()> {
            conn.execute("CREATE TABLE users (id BIGINT PRIMARY KEY);", NO_PARAMS).map(|_| ())
        }
        
        fn down(&self, conn: &SqliteConnection) -> SqliteResult<()> {
            conn.execute("DROP TABLE users;", NO_PARAMS).map(|_| ())
        }
    }

    #[test]
    pub fn test_register() {
        let conn = Rc::new(RefCell::new(SqliteConnection::open_in_memory().unwrap()));

        let adapter = SqliteAdapter::new(conn);

        adapter.setup_schema();

        let mut migrator = Migrator::new(adapter);

        migrator.register(Box::new(CreateUsers));

        migrator.up(Some(1)).unwrap();

        assert_eq!(migrator.current_version().unwrap(), Some(1));

        migrator.down(None).unwrap();
        
        assert_eq!(migrator.current_version().unwrap(), None);
    }
}
