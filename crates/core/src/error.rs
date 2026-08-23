//! Unified error type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MdbError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Opaque storage-layer error (redb), boxed so `core` stays independent
    /// of the storage backend; the original is reachable via `source()`.
    #[error("storage: {0}")]
    Store(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("bad query: {0}")]
    BadQuery(String),

    /// Opaque search-layer error (tantivy).
    #[error("search: {0}")]
    Search(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("not found: {0}")]
    NotFound(String),
}

impl MdbError {
    /// Wraps an opaque storage-layer error.
    pub fn store(e: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Store(Box::new(e))
    }

    /// Wraps an opaque search-layer error.
    pub fn search(e: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Search(Box::new(e))
    }
}
