//! tantivy full-text index: `id` (exact-match u64) + `body` (text).

use std::path::Path;
use std::sync::Mutex;

use core::error::MdbError;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, INDEXED, STORED, TEXT};
use tantivy::{Index, IndexReader, IndexWriter, TantivyDocument, Term};

/// Full-text index over document bodies, keyed by exact-match numeric ids.
pub struct Search {
    // IndexWriter::commit takes &mut self; wrapping keeps the &self API.
    writer: Mutex<IndexWriter>,
    reader: IndexReader,
    parser: QueryParser,
    id: Field,
    body: Field,
}

impl Search {
    pub fn open(path: &Path) -> Result<Self, MdbError> {
        let mut sb = Schema::builder();
        let id = sb.add_u64_field("id", INDEXED | STORED);
        let body = sb.add_text_field("body", TEXT);
        let schema = sb.build();

        std::fs::create_dir_all(path)?;
        let index = if path.join("meta.json").exists() {
            Index::open_in_dir(path).map_err(MdbError::search)?
        } else {
            Index::create_in_dir(path, schema).map_err(MdbError::search)?
        };
        let parser = QueryParser::for_index(&index, vec![body]);
        let writer = Mutex::new(
            index
                .writer_with_num_threads(1, 30_000_000)
                .map_err(MdbError::search)?,
        );
        let reader = index.reader().map_err(MdbError::search)?;
        Ok(Self {
            writer,
            reader,
            parser,
            id,
            body,
        })
    }

    /// Indexes `id`/`body`, replacing any previous version of `id`.
    pub fn upsert(&self, id: u64, body: &str) -> Result<(), MdbError> {
        let mut writer = self.writer.lock().unwrap();
        writer.delete_term(self.term(id));
        let mut doc = TantivyDocument::default();
        doc.add_u64(self.id, id);
        doc.add_text(self.body, body);
        writer.add_document(doc).map_err(MdbError::search)?;
        commit_and_reload(&mut writer, &self.reader)
    }

    pub fn delete(&self, id: u64) -> Result<(), MdbError> {
        let mut writer = self.writer.lock().unwrap();
        writer.delete_term(self.term(id));
        commit_and_reload(&mut writer, &self.reader)
    }

    /// Returns `(id, score)` pairs, best match first.
    pub fn query(&self, q: &str, limit: usize) -> Result<Vec<(u64, f32)>, MdbError> {
        let query = self
            .parser
            .parse_query(q)
            .map_err(|e| MdbError::BadQuery(e.to_string()))?;
        let searcher = self.reader.searcher();
        let top = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .map_err(MdbError::search)?;
        top.into_iter()
            .map(|(score, addr)| {
                let doc: TantivyDocument = searcher.doc(addr).map_err(MdbError::search)?;
                let id = doc
                    .get_first(self.id)
                    .and_then(|v| v.as_u64())
                    .unwrap_or_default();
                Ok((id, score))
            })
            .collect()
    }

    fn term(&self, id: u64) -> Term {
        Term::from_field_u64(self.id, id)
    }
}

fn commit_and_reload(writer: &mut IndexWriter, reader: &IndexReader) -> Result<(), MdbError> {
    writer.commit().map_err(MdbError::search)?;
    reader.reload().map_err(MdbError::search)?;
    Ok(())
}
