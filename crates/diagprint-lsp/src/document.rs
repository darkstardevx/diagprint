use lsp_types::Uri;
use std::collections::BTreeMap;

/// LSP identity associated with one diagprint source.
///
/// The external LSP document version deliberately remains separate from
/// `diagprint::SourceRevision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspDocument {
    pub uri: Uri,
    pub version: Option<i32>,
}

impl LspDocument {
    pub fn versioned(uri: Uri, version: i32) -> Self {
        Self {
            uri,
            version: Some(version),
        }
    }

    pub fn unversioned(uri: Uri) -> Self {
        Self { uri, version: None }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DocumentMap {
    documents: BTreeMap<String, LspDocument>,
}

impl DocumentMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        source: impl Into<String>,
        document: LspDocument,
    ) -> Option<LspDocument> {
        self.documents.insert(source.into(), document)
    }

    pub fn insert_versioned(
        &mut self,
        source: impl Into<String>,
        uri: Uri,
        version: i32,
    ) -> Option<LspDocument> {
        self.insert(source, LspDocument::versioned(uri, version))
    }

    pub fn insert_unversioned(
        &mut self,
        source: impl Into<String>,
        uri: Uri,
    ) -> Option<LspDocument> {
        self.insert(source, LspDocument::unversioned(uri))
    }

    pub fn get(&self, source: &str) -> Option<&LspDocument> {
        self.documents.get(source)
    }

    pub fn remove(&mut self, source: &str) -> Option<LspDocument> {
        self.documents.remove(source)
    }

    pub fn contains(&self, source: &str) -> bool {
        self.documents.contains_key(source)
    }

    pub fn len(&self) -> usize {
        self.documents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }
}
