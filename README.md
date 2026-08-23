# markv

Database for markdown files: embedded storage ([redb]) + full-text search ([tantivy]), served over HTTP.

- `crates/core` — common types (`Document`, `SearchHit`, `MdbError`)
- `crates/storage` — redb-backed key/value store
- `crates/search` — tantivy full-text index
- `crates/shard` — shard identity/routing seam + engine assembling per-shard storage/search pairs
- `crates/api` — axum HTTP handlers
- `crates/bin/markv` — HTTP server binary owning the data directory
- `crates/bin/cli` — CLI client for a running server.
- `tests/` — integration tests (engine roundtrip), `crates/bin/cli/tests/` — HTTP API roundtrip.

## Usage

```console
$ markv ./data                             # start server on 127.0.0.1:9379
$ markv /data --addr 0.0.0.0:9379          # custom bind address

# client (--addr optional, default 127.0.0.1:9379)
$ cli put note.md              # server assigns a snowflake id (printed)
$ cli put note.md my-old-id    # deterministic upsert by explicit id
$ cli get <base62-id>
$ cli search "full text"
$ cli ls
$ cli rm <base62-id>
```

Ids are u64 snowflakes ([beakid]); the wire format is base62 strings.

## HTTP API

```text
GET    /docs               -> ["<base62-id>", …]
PUT    /docs               body: markdown -> {"id":"<assigned>"}
PUT    /docs?id=<base62>   body: markdown -> deterministic upsert
GET    /docs?id=<base62>   -> markdown (404 when absent)
DELETE /docs?id=<base62>   -> 204 | 404
GET    /search?q=…&limit=N -> [{"id","score"}]
```

Server flags: `--addr`, `--worker-id` (unique per instance).

## Development

```console
$ nix develop        # rust toolchain, rustfmt, clippy, rust-analyzer
$ cargo test
```
