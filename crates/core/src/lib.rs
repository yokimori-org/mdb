//! Common data types shared across markv crates.

pub mod error;

pub use error::MdbError;

/// A stored markdown document. Ids are u64 snowflakes (crate `beakid`).
#[derive(Debug, Clone)]
pub struct Document {
    pub id: u64,
    pub content: String,
}

/// A search result.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub id: u64,
    pub score: f32,
}
