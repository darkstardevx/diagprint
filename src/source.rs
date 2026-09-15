//! Shared in-memory source cache.
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
    sync::{Arc, RwLock},
};

/// Thread-safe shared cache of named source texts.
///
/// Cloning a `SourceCache` is cheap: clones share the same underlying cache.
/// A type which can supply named source text to a [`SourceCache`].
///
/// This is intentionally independent of any particular diagnostic ecosystem.
/// Parsers, compilers, editor integrations, and third-party bridges can expose
/// their in-memory source text through the same interface.
pub trait SourceProvider {
    /// Adds this provider's current sources to `cache`.
    ///
    /// Existing entries with the same exact source name are replaced.
    fn populate_source_cache(&self, cache: &SourceCache);

    /// Builds a new cache containing this provider's current sources.
    fn source_cache(&self) -> SourceCache {
        let cache = SourceCache::new();
        self.populate_source_cache(&cache);
        cache
    }
}

#[derive(Debug, Clone, Default)]
pub struct SourceCache {
    inner: Arc<RwLock<BTreeMap<String, Arc<str>>>>,
}

impl SourceCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or replaces a source.
    ///
    /// Returns the previous source if one existed under the same exact name.
    pub fn insert(&self, name: impl Into<String>, source: impl Into<String>) -> Option<Arc<str>> {
        let source: String = source.into();

        self.write().insert(name.into(), Arc::<str>::from(source))
    }

    /// Returns a shared reference-counted copy of a cached source.
    pub fn get(&self, name: &str) -> Option<Arc<str>> {
        self.read().get(name).cloned()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.read().contains_key(name)
    }

    pub fn remove(&self, name: &str) -> Option<Arc<str>> {
        self.write().remove(name)
    }

    pub fn clear(&self) {
        self.write().clear();
    }

    pub fn len(&self) -> usize {
        self.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.read().is_empty()
    }

    pub fn names(&self) -> Vec<String> {
        self.read().keys().cloned().collect()
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, BTreeMap<String, Arc<str>>> {
        self.inner
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, BTreeMap<String, Arc<str>>> {
        self.inner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
