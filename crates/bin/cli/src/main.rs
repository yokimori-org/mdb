//! CLI client of the markv server (`markv <DB_DIR>`).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use cli::client::Client;

#[derive(Parser)]
#[command(
    name = "cli",
    version,
    about = "CLI client for the markv markdown database server"
)]
struct Cli {
    /// Server address
    #[arg(long, global = true, default_value = "127.0.0.1:9379")]
    addr: String,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Store a markdown file (omit id to get a server-assigned one)
    Put {
        file: PathBuf,
        /// Base62 document id
        id: Option<String>,
    },
    /// Print a document
    Get { id: String },
    /// Delete a document
    Rm { id: String },
    /// List document ids
    Ls,
    /// Full-text search
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}

fn fail(e: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {e}");
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let args = Cli::parse();
    let client = Client::new(&args.addr);
    match args.cmd {
        Cmd::Put { id, file } => match std::fs::read_to_string(&file) {
            Ok(content) => match client.put(id.as_deref(), &content) {
                Ok(assigned) => {
                    println!("{assigned}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            },
            Err(e) => fail(e),
        },
        Cmd::Get { id } => match client.get(&id) {
            Ok(doc) => {
                print!("{doc}");
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        Cmd::Rm { id } => match client.rm(&id) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => fail("not found"),
            Err(e) => fail(e),
        },
        Cmd::Ls => match client.ls() {
            Ok(ids) => {
                for id in ids {
                    println!("{id}");
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        Cmd::Search { query, limit } => match client.search(&query, limit) {
            Ok(hits) => {
                for h in hits {
                    println!("{}\t{:.4}", h.id, h.score);
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
    }
}
