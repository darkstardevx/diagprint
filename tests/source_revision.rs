use diagprint::SourceCache;

#[test]
fn revisions_increment_for_repeated_insertions() {
    const NAME: &str = "memory://editor/main.rs";

    let cache = SourceCache::new();

    cache.insert(NAME, "revision one\n");
    let first = cache.revision(NAME).unwrap();

    cache.insert(NAME, "revision two\n");
    let second = cache.revision(NAME).unwrap();

    cache.insert(NAME, "revision three\n");
    let third = cache.revision(NAME).unwrap();

    assert_eq!(first.get(), 1);
    assert_eq!(second.get(), 2);
    assert_eq!(third.get(), 3);

    assert!(first < second);
    assert!(second < third);
}

#[test]
fn source_names_have_independent_revision_sequences() {
    let cache = SourceCache::new();

    cache.insert("one.rs", "one\n");
    cache.insert("two.rs", "two\n");

    assert_eq!(cache.revision("one.rs").unwrap().get(), 1);

    assert_eq!(cache.revision("two.rs").unwrap().get(), 1);

    cache.insert("one.rs", "one updated\n");

    assert_eq!(cache.revision("one.rs").unwrap().get(), 2);

    assert_eq!(cache.revision("two.rs").unwrap().get(), 1);
}

#[test]
fn snapshot_preserves_revision_and_text() {
    const NAME: &str = "memory://snapshot/main.rs";

    let cache = SourceCache::new();

    cache.insert(NAME, "old source\n");

    let snapshot = cache.snapshot();
    let captured_revision = snapshot.revision(NAME).unwrap();

    assert_eq!(captured_revision.get(), 1);
    assert_eq!(snapshot.get(NAME).as_deref(), Some("old source\n"));

    cache.insert(NAME, "new source\n");

    assert_eq!(cache.revision(NAME).unwrap().get(), 2);

    assert_eq!(snapshot.revision(NAME), Some(captured_revision));

    assert_eq!(snapshot.get(NAME).as_deref(), Some("old source\n"));
}

#[test]
fn snapshot_detects_live_revision_changes() {
    const NAME: &str = "memory://editor/stale.rs";

    let cache = SourceCache::new();

    cache.insert(NAME, "first\n");

    let snapshot = cache.snapshot();

    assert!(snapshot.is_current(&cache, NAME));
    assert!(!snapshot.is_stale(&cache, NAME));

    cache.insert(NAME, "second\n");

    assert!(!snapshot.is_current(&cache, NAME));
    assert!(snapshot.is_stale(&cache, NAME));
}

#[test]
fn removal_makes_snapshot_stale() {
    const NAME: &str = "memory://editor/removed.rs";

    let cache = SourceCache::new();

    cache.insert(NAME, "present\n");

    let snapshot = cache.snapshot();

    assert!(snapshot.is_current(&cache, NAME));

    cache.remove(NAME);

    assert!(!snapshot.is_current(&cache, NAME));
    assert!(snapshot.is_stale(&cache, NAME));
}

#[test]
fn remove_and_reinsert_does_not_reuse_revision() {
    const NAME: &str = "memory://editor/reinsert.rs";

    let cache = SourceCache::new();

    cache.insert(NAME, "first\n");

    let old_revision = cache.revision(NAME).unwrap();

    cache.remove(NAME);

    cache.insert(NAME, "second\n");

    let new_revision = cache.revision(NAME).unwrap();

    assert_eq!(old_revision.get(), 1);
    assert_eq!(new_revision.get(), 2);
    assert_ne!(old_revision, new_revision);
}

#[test]
fn clear_does_not_reset_revision_history() {
    const NAME: &str = "memory://editor/clear.rs";

    let cache = SourceCache::new();

    cache.insert(NAME, "first\n");

    assert_eq!(cache.revision(NAME).unwrap().get(), 1);

    cache.clear();

    assert!(cache.revision(NAME).is_none());

    cache.insert(NAME, "second\n");

    assert_eq!(cache.revision(NAME).unwrap().get(), 2);
}

#[test]
fn insert_revisioned_returns_the_new_revision() {
    const NAME: &str = "memory://editor/direct.rs";

    let cache = SourceCache::new();

    let first = cache.insert_revisioned(NAME, "first\n");

    let second = cache.insert_revisioned(NAME, "second\n");

    assert_eq!(first.get(), 1);
    assert_eq!(second.get(), 2);
    assert_eq!(cache.revision(NAME), Some(second));
}
