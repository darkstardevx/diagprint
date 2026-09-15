//! Shared in-memory source storage.
//!
//! `SourceCache` lets diagnostics render source text which does not exist on
//! disk: editor buffers, generated files, parser inputs, temporary documents,
//! compiler virtual files, and other in-memory sources.
//!
//! Source names are matched exactly. No filesystem canonicalization is
//! performed because virtual names do not necessarily represent paths.
//!
//! Cached source text is deliberately kept outside [`crate::Diagnostic`].
//! It is therefore not serialized into JSON reports or persisted as part of
//! diagnostic metadata.

use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, RwLock},
};

/// Monotonic revision of one named source.
///
/// Revisions begin at `1` when a source name is first inserted into a
/// [`SourceCache`]. Replacing that source increments its revision.
///
/// Revision histories survive [`SourceCache::remove`] and
/// [`SourceCache::clear`], preventing a removed and later reinserted source
/// from accidentally reusing an older revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceRevision(u64);

impl SourceRevision {
    /// Returns the numeric revision.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SourceRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// One immutable source revision.
#[derive(Debug, Clone)]
pub struct SourceEntry {
    revision: SourceRevision,
    text: Arc<str>,
}

impl SourceEntry {
    /// Revision represented by this entry.
    pub const fn revision(&self) -> SourceRevision {
        self.revision
    }

    /// Source text represented by this entry.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns a shared reference-counted handle to the source text.
    pub fn shared_text(&self) -> Arc<str> {
        self.text.clone()
    }
}

/// A type which can supply named source text to a [`SourceCache`].
///
/// This is intentionally independent of any particular diagnostic ecosystem.
/// Parsers, compilers, editor integrations, and third-party bridges can expose
/// their in-memory source text through the same interface.
pub trait SourceProvider {
    /// Adds this provider's current sources to `cache`.
    ///
    /// Existing entries with the same exact source name are replaced and
    /// therefore receive new revisions.
    fn populate_source_cache(&self, cache: &SourceCache);

    /// Builds a new cache containing this provider's current sources.
    fn source_cache(&self) -> SourceCache {
        let cache = SourceCache::new();
        self.populate_source_cache(&cache);
        cache
    }
}

/// Immutable point-in-time view of a [`SourceCache`].
///
/// A snapshot keeps both the source text and its revision as they existed when
/// [`SourceCache::snapshot`] was called, even if the live cache is subsequently
/// updated, cleared, or has entries removed.
///
/// Source text is reference counted, so creating a snapshot does not duplicate
/// the underlying strings.
#[derive(Debug, Clone, Default)]
pub struct SourceSnapshot {
    inner: Arc<BTreeMap<String, SourceEntry>>,
}

impl SourceSnapshot {
    /// Returns source text captured by this snapshot.
    pub fn get(&self, name: &str) -> Option<Arc<str>> {
        self.inner.get(name).map(SourceEntry::shared_text)
    }

    /// Returns the complete captured source entry.
    pub fn entry(&self, name: &str) -> Option<SourceEntry> {
        self.inner.get(name).cloned()
    }

    /// Returns the captured revision for `name`.
    pub fn revision(&self, name: &str) -> Option<SourceRevision> {
        self.inner.get(name).map(SourceEntry::revision)
    }

    /// Returns whether this snapshot's source is still the current live
    /// revision in `cache`.
    ///
    /// A source absent from this snapshot is never considered current.
    pub fn is_current(&self, cache: &SourceCache, name: &str) -> bool {
        match self.revision(name) {
            Some(revision) => cache.revision(name) == Some(revision),
            None => false,
        }
    }

    /// Returns whether this snapshot contains `name` but the live cache no
    /// longer has the same revision.
    pub fn is_stale(&self, cache: &SourceCache, name: &str) -> bool {
        self.contains(name) && !self.is_current(cache, name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.inner.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn names(&self) -> Vec<String> {
        self.inner.keys().cloned().collect()
    }
}

#[derive(Debug, Default)]
struct SourceCacheState {
    entries: BTreeMap<String, SourceEntry>,

    // Revision counters deliberately survive entry removal and clear().
    revisions: BTreeMap<String, u64>,
}

impl SourceCacheState {
    fn next_revision(&mut self, name: &str) -> SourceRevision {
        let revision = self.revisions.entry(name.to_owned()).or_insert(0);

        *revision = revision
            .checked_add(1)
            .expect("diagprint source revision counter overflowed u64");

        SourceRevision(*revision)
    }
}

/// Thread-safe shared cache of named source texts.
///
/// Cloning a `SourceCache` is cheap: clones share the same underlying cache.
#[derive(Debug, Clone, Default)]
pub struct SourceCache {
    inner: Arc<RwLock<SourceCacheState>>,
}

impl SourceCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Captures an immutable point-in-time view of the current cache.
    ///
    /// Source strings are shared through `Arc`, so snapshotting copies source
    /// metadata but not the underlying source text.
    pub fn snapshot(&self) -> SourceSnapshot {
        SourceSnapshot {
            inner: Arc::new(self.read().entries.clone()),
        }
    }

    /// Inserts or replaces a source.
    ///
    /// Every insertion receives a new revision for that source name.
    ///
    /// Returns the previous source text if one existed under the same exact
    /// name.
    pub fn insert(&self, name: impl Into<String>, source: impl Into<String>) -> Option<Arc<str>> {
        let name = name.into();
        let source: Arc<str> = Arc::from(source.into());

        let mut state = self.write();
        let revision = state.next_revision(&name);

        state
            .entries
            .insert(
                name,
                SourceEntry {
                    revision,
                    text: source,
                },
            )
            .map(|entry| entry.text)
    }

    /// Inserts or replaces a source and returns its new revision.
    ///
    /// This is useful for editor and language-server integrations which need
    /// to associate work directly with a specific source revision.
    pub fn insert_revisioned(
        &self,
        name: impl Into<String>,
        source: impl Into<String>,
    ) -> SourceRevision {
        let name = name.into();
        let source: Arc<str> = Arc::from(source.into());

        let mut state = self.write();
        let revision = state.next_revision(&name);

        state.entries.insert(
            name,
            SourceEntry {
                revision,
                text: source,
            },
        );

        revision
    }

    /// Returns a shared reference-counted copy of current source text.
    pub fn get(&self, name: &str) -> Option<Arc<str>> {
        self.read().entries.get(name).map(SourceEntry::shared_text)
    }

    /// Returns the complete current source entry.
    pub fn entry(&self, name: &str) -> Option<SourceEntry> {
        self.read().entries.get(name).cloned()
    }

    /// Returns the current revision for a cached source.
    pub fn revision(&self, name: &str) -> Option<SourceRevision> {
        self.read().entries.get(name).map(SourceEntry::revision)
    }

    /// Returns whether `revision` is currently active for `name`.
    pub fn is_revision_current(&self, name: &str, revision: SourceRevision) -> bool {
        self.revision(name) == Some(revision)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.read().entries.contains_key(name)
    }

    /// Removes a live source while preserving its revision history.
    pub fn remove(&self, name: &str) -> Option<Arc<str>> {
        self.write().entries.remove(name).map(|entry| entry.text)
    }

    /// Removes every live source while preserving revision histories.
    ///
    /// If a cleared source name is later reinserted, its revision continues
    /// from the previous value instead of restarting at `1`.
    pub fn clear(&self) {
        self.write().entries.clear();
    }

    pub fn len(&self) -> usize {
        self.read().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.read().entries.is_empty()
    }

    pub fn names(&self) -> Vec<String> {
        self.read().entries.keys().cloned().collect()
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, SourceCacheState> {
        self.inner
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, SourceCacheState> {
        self.inner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
