//! redb-backed document storage: `docs` table, key = snowflake id, value = markdown.

use std::path::Path;

use core::error::MdbError;
use redb::{Database, TableDefinition};

const DOCS: TableDefinition<u64, &str> = TableDefinition::new("docs");

/// Key/value document store (snowflake id → markdown).
pub struct Store {
    db: Database,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, MdbError> {
        Ok(Self {
            db: Database::create(path).map_err(MdbError::store)?,
        })
    }

    pub fn put(&self, id: u64, content: &str) -> Result<(), MdbError> {
        let tx = self.db.begin_write().map_err(MdbError::store)?;
        {
            let mut table = tx.open_table(DOCS).map_err(MdbError::store)?;
            table.insert(id, content).map_err(MdbError::store)?;
        }
        tx.commit().map_err(MdbError::store)?;
        Ok(())
    }

    pub fn get(&self, id: u64) -> Result<Option<String>, MdbError> {
        let tx = self.db.begin_read().map_err(MdbError::store)?;
        let table = match tx.open_table(DOCS) {
            // fresh database: no writes yet -> table does not exist
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            other => other.map_err(MdbError::store)?,
        };
        match table.get(id).map_err(MdbError::store)? {
            Some(v) => Ok(Some(v.value().to_owned())),
            None => Ok(None),
        }
    }

    /// Removes `id`; returns true when it existed.
    pub fn delete(&self, id: u64) -> Result<bool, MdbError> {
        let tx = self.db.begin_write().map_err(MdbError::store)?;
        let removed = {
            let mut table = tx.open_table(DOCS).map_err(MdbError::store)?;
            let removed = table.remove(id).map_err(MdbError::store)?.is_some();
            removed
        };
        tx.commit().map_err(MdbError::store)?;
        Ok(removed)
    }

    /// All document ids, ordered by key (snowflake order = chronological).
    pub fn ids(&self) -> Result<Vec<u64>, MdbError> {
        let tx = self.db.begin_read().map_err(MdbError::store)?;
        let table = match tx.open_table(DOCS) {
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            other => other.map_err(MdbError::store)?,
        };
        let mut out = Vec::new();
        for row in table.range::<u64>(..).map_err(MdbError::store)? {
            let (k, _) = row.map_err(MdbError::store)?;
            out.push(k.value());
        }
        Ok(out)
    }
}
