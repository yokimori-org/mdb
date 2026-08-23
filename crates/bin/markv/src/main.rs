//! `markv` — HTTP server owning a data directory.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use api::AppState;
use clap::Parser;
use shard::Engine;

#[derive(Parser)]
#[command(
    name = "markv",
    version,
    about = "Markdown database server: redb storage + tantivy search over HTTP"
)]
struct Args {
    /// Data directory (created if missing)
    db_dir: PathBuf,

    /// Address to bind
    #[arg(long, default_value = "127.0.0.1:9379")]
    addr: String,

    /// Snowflake worker id — must be unique per markv instance
    #[arg(long, default_value_t = 0)]
    worker_id: u16,
}

fn fail(e: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {e}");
    ExitCode::FAILURE
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let engine = match Engine::open(&args.db_dir) {
        Ok(e) => Arc::new(e),
        Err(e) => return fail(e),
    };
    let listener = match tokio::net::TcpListener::bind(&args.addr).await {
        Ok(l) => l,
        Err(e) => return fail(e),
    };
    eprintln!("markv listening on http://{}", args.addr);
    api::serve(Arc::new(AppState::new(engine, args.worker_id)), listener)
        .await
        .map_or_else(fail, |_| ExitCode::SUCCESS)
}
