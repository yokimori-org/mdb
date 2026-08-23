use cli::client::{Client, ClientError};
use shard::Engine;

// ponytail: blocking ureq needs a second worker thread, or the spawned
// server task never runs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let engine = std::sync::Arc::new(Engine::open(dir.path()).unwrap());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        api::serve(std::sync::Arc::new(api::AppState::new(engine, 0)), listener)
            .await
            .unwrap();
    });

    let c = Client::new(&addr.to_string());

    // put without id -> server-assigned base62 snowflake, unique per call
    let first = c
        .put(None, "# Intro\n\ntantivy full-text search rocks\n")
        .unwrap();
    let second = c
        .put(None, "plain notes about redb storage engine\n")
        .unwrap();
    assert_ne!(first, second);

    // explicit base62 id -> deterministic upsert
    c.put(Some("settings"), "stable config\n").unwrap();
    assert_eq!(c.get("settings").unwrap(), "stable config\n");

    // get by generated id; unknown but valid id -> NotFound
    assert_eq!(
        c.get(&first).unwrap(),
        "# Intro\n\ntantivy full-text search rocks\n"
    );
    assert!(matches!(c.get("2Zq8z1Kf"), Err(ClientError::NotFound)));

    // invalid base62 -> 400 from the server
    assert!(matches!(c.get("not valid!"), Err(ClientError::Http(_))));

    // list + search over base62 ids
    assert!(c.ls().unwrap().contains(&first));
    assert!(c.ls().unwrap().contains(&"settings".to_string()));
    assert_eq!(c.search("tantivy", 10).unwrap()[0].id, first);
    assert!(c.search("qqqqzz", 10).unwrap().is_empty());

    // overwrite explicit id keeps exactly one indexed copy
    c.put(Some("settings"), "rewritten config about databases\n")
        .unwrap();
    assert_eq!(c.search("rewritten", 10).unwrap()[0].id, "settings");
    assert!(c.search("stable", 10).unwrap().is_empty());

    // delete semantics
    assert!(c.rm("settings").unwrap());
    assert!(!c.rm("settings").unwrap());
    assert!(matches!(c.get("settings"), Err(ClientError::NotFound)));
    assert!(c.search("databases", 10).unwrap().is_empty());
    assert_eq!(c.search("storage", 10).unwrap()[0].id, second);
}
