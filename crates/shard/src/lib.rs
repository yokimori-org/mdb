//! Sharding: [`ShardId`] and the routing seam, plus the [`Engine`] that owns
//! one storage+search pair per shard and fans operations out across them.

use std::collections::HashMap;
use std::path::Path;

use core::error::MdbError;
pub use core::{Document, SearchHit};
use search::Search;
use storage::Store;

/// Identifies one shard: a self-contained `<root>/shards/<NNNN>/` directory
/// holding its own redb database and tantivy index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShardId(pub u32);

impl ShardId {
    /// Zero-padded directory name, sorts naturally.
    pub fn dir_name(self) -> String {
        format!("{:04}", self.0)
    }
}

impl std::fmt::Display for ShardId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shard-{:04}", self.0)
    }
}

/// Maps a document id to the shard owning it.
///
/// ponytail: always shard 0 — replace with `id % shard_count` or consistent
/// hashing when real sharding lands. The engine already keys everything by
/// `ShardId`, so this is the only function that has to change.
pub fn route(_doc_id: u64) -> ShardId {
    ShardId(0)
}

struct Shard {
    store: Store,
    search: Search,
}

/// Embedded markdown database: storage (redb) + full-text search (tantivy).
pub struct Engine {
    shards: HashMap<ShardId, Shard>,
}

impl Engine {
    /// Opens the database at `root`, creating it if needed.
    ///
    /// Layout: `<root>/shards/<NNNN>/` — one self-contained directory per
    /// shard (`data.redb` + `index/`).
    pub fn open(root: impl AsRef<Path>) -> Result<Self, MdbError> {
        let root = root.as_ref();
        // ponytail: single shard — the per-ShardId map is already the seam,
        // loop over configured shards when routing lands.
        let mut shards = HashMap::new();
        {
            let id = route(0);
            let dir = root.join("shards").join(id.dir_name());
            std::fs::create_dir_all(&dir)?;
            shards.insert(
                id,
                Shard {
                    store: Store::open(&dir.join("data.redb"))?,
                    search: Search::open(&dir.join("index"))?,
                },
            );
        }
        Ok(Self { shards })
    }

    fn shard_for(&self, doc_id: u64) -> Result<&Shard, MdbError> {
        self.shards
            .get(&route(doc_id))
            .ok_or_else(|| MdbError::NotFound(format!("no shard for {doc_id}")))
    }

    /// Inserts or overwrites a document and updates the search index.
    ///
    /// Data is committed first: if indexing fails, the document is still
    /// stored and a repeated `put` repairs the index.
    pub fn put(&self, id: u64, content: &str) -> Result<(), MdbError> {
        let s = self.shard_for(id)?;
        s.store.put(id, content)?;
        s.search.upsert(id, content)?;
        Ok(())
    }

    /// Fetches a document by id.
    pub fn get(&self, id: u64) -> Result<Document, MdbError> {
        let s = self.shard_for(id)?;
        s.store
            .get(id)?
            .map(|content| Document { id, content })
            .ok_or_else(|| MdbError::NotFound(format!("{id}")))
    }

    /// Deletes a document from both store and index; returns true when it existed.
    pub fn delete(&self, id: u64) -> Result<bool, MdbError> {
        let s = self.shard_for(id)?;
        let existed = s.store.delete(id)?;
        if existed {
            s.search.delete(id)?;
        }
        Ok(existed)
    }

    /// All document ids, sorted — snowflake order is chronological.
    pub fn ids(&self) -> Result<Vec<u64>, MdbError> {
        let mut out = Vec::new();
        for s in self.shards.values() {
            out.extend(s.store.ids()?);
        }
        out.sort();
        Ok(out)
    }

    /// Full-text search over document bodies, best match first.
    /// Results are merged across shards and cut to `limit`.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, MdbError> {
        let mut out = Vec::new();
        for s in self.shards.values() {
            out.extend(
                s.search
                    .query(query, limit)?
                    .into_iter()
                    .map(|(id, score)| SearchHit { id, score }),
            );
        }
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out.truncate(limit);
        Ok(out)
    }
}
