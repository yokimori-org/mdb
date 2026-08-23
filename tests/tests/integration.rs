use core::MdbError;
use shard::Engine;

#[test]
fn put_get_search_delete_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let eng = Engine::open(dir.path()).unwrap();

    // put + get
    eng.put(101, "# Intro\n\ntantivy full-text search rocks\n")
        .unwrap();
    eng.put(202, "plain notes about redb storage engine\n")
        .unwrap();
    assert_eq!(
        eng.get(101).unwrap().content,
        "# Intro\n\ntantivy full-text search rocks\n"
    );
    assert!(matches!(eng.get(999), Err(MdbError::NotFound(_))));

    // search finds the right doc; unknown terms find nothing
    let hits = eng.search("tantivy", 10).unwrap();
    assert_eq!(hits.iter().map(|h| h.id).collect::<Vec<_>>(), [101]);
    assert!(eng.search("qqqqzz", 10).unwrap().is_empty());

    // overwrite: store replaced, exactly one indexed copy, old words gone
    eng.put(101, "# Intro\n\nrewritten body about databases\n")
        .unwrap();
    assert!(eng.get(101).unwrap().content.contains("rewritten"));
    let hits = eng.search("rewritten", 10).unwrap();
    assert_eq!(hits.iter().filter(|h| h.id == 101).count(), 1);
    assert!(eng.search("rocks", 10).unwrap().is_empty());

    // survives reopen: redb + tantivy state on disk
    drop(eng);
    let eng = Engine::open(dir.path()).unwrap();
    assert!(eng.get(101).unwrap().content.contains("rewritten"));
    assert_eq!(eng.ids().unwrap(), [101, 202]);
    assert_eq!(eng.search("databases", 10).unwrap()[0].id, 101);

    // delete: gone from store and index; double delete reported
    assert!(eng.delete(101).unwrap());
    assert!(!eng.delete(101).unwrap());
    assert!(matches!(eng.get(101), Err(MdbError::NotFound(_))));
    assert!(eng.search("databases", 10).unwrap().is_empty());
    assert_eq!(eng.search("storage", 10).unwrap()[0].id, 202);
}
