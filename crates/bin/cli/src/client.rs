//! Blocking HTTP client for the markv server (used by this CLI).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Wire format of `GET /search` hits.
#[derive(Debug, Deserialize)]
struct PutResp {
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub id: String,
    pub score: f32,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("server: {0}")]
    Http(String),
    #[error("not found")]
    NotFound,
    #[error("bad response: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    // boxed: ureq::Transport is ~200B and would trip clippy result_large_err
    #[error("connection: {0}")]
    Transport(#[from] Box<ureq::Transport>),
}

pub struct Client {
    base: String,
}

impl Client {
    pub fn new(addr: &str) -> Self {
        Self {
            base: format!("http://{addr}"),
        }
    }

    fn resp(r: Result<ureq::Response, ureq::Error>) -> Result<ureq::Response, ClientError> {
        match r {
            Ok(resp) => Ok(resp),
            Err(ureq::Error::Status(404, _)) => Err(ClientError::NotFound),
            Err(ureq::Error::Status(code, resp)) => {
                Err(ClientError::Http(match resp.into_string() {
                    Ok(body) => format!("{code}: {body}"),
                    Err(_) => code.to_string(),
                }))
            }
            Err(ureq::Error::Transport(t)) => Err(ClientError::from(Box::new(t))),
        }
    }

    /// Stores or replaces a document. Without `id` the server assigns a
    /// snowflake; returns the id in effect, base62-encoded.
    pub fn put(&self, id: Option<&str>, content: &str) -> Result<String, ClientError> {
        let mut req = ureq::put(&format!("{}/docs", self.base));
        if let Some(id) = id {
            req = req.query("id", id);
        }
        let resp = Self::resp(req.send_string(content))?;
        Ok(resp.into_json::<PutResp>()?.id)
    }

    pub fn get(&self, id: &str) -> Result<String, ClientError> {
        let resp = Self::resp(
            ureq::get(&format!("{}/docs", self.base))
                .query("id", id)
                .call(),
        )?;
        Ok(resp.into_string()?)
    }

    /// Deletes a document; returns false when it did not exist.
    pub fn rm(&self, id: &str) -> Result<bool, ClientError> {
        match Self::resp(
            ureq::delete(&format!("{}/docs", self.base))
                .query("id", id)
                .call(),
        ) {
            Ok(_) => Ok(true),
            Err(ClientError::NotFound) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn ls(&self) -> Result<Vec<String>, ClientError> {
        let resp = Self::resp(ureq::get(&format!("{}/docs", self.base)).call())?;
        Ok(resp.into_json()?)
    }

    pub fn search(&self, q: &str, limit: usize) -> Result<Vec<Hit>, ClientError> {
        let resp = Self::resp(
            ureq::get(&format!("{}/search", self.base))
                .query("q", q)
                .query("limit", &limit.to_string())
                .call(),
        )?;
        Ok(resp.into_json()?)
    }
}
