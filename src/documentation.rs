//! Version-aware documentation resolution.
//!
//! The resolver can pin:
//!
//! - Rust standard-library documentation to a specific toolchain version;
//! - Rust compiler error documentation to a specific toolchain version;
//! - rustc and Cargo documentation to a specific Rust release;
//! - docs.rs links to package versions discovered from `Cargo.lock`.
//!
//! Package-version ambiguity is never resolved by guessing.

use crate::DocumentationLink;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs, io,
    path::Path,
};

#[derive(Debug)]
pub enum DocumentationError {
    Io(io::Error),

    Parse(toml::de::Error),

    UnknownPackage {
        package: String,
    },

    AmbiguousPackage {
        package: String,
        versions: Vec<String>,
    },
}

impl fmt::Display for DocumentationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => {
                write!(f, "could not read documentation metadata: {error}")
            }

            Self::Parse(error) => {
                write!(f, "could not parse Cargo.lock: {error}")
            }

            Self::UnknownPackage { package } => {
                write!(
                    f,
                    "package `{package}` is not present in the documentation catalog"
                )
            }

            Self::AmbiguousPackage { package, versions } => {
                write!(
                    f,
                    "package `{package}` has multiple locked versions: {}",
                    versions.join(", ")
                )
            }
        }
    }
}

impl Error for DocumentationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::UnknownPackage { .. } | Self::AmbiguousPackage { .. } => None,
        }
    }
}

impl From<io::Error> for DocumentationError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for DocumentationError {
    fn from(value: toml::de::Error) -> Self {
        Self::Parse(value)
    }
}

/// Resolves documentation without silently guessing package versions.
///
/// A resolver may be populated manually or from a Cargo lockfile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentationResolver {
    rust_version: Option<String>,

    package_versions: BTreeMap<String, BTreeSet<String>>,
}

impl DocumentationResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a documentation catalog from an existing `Cargo.lock`.
    pub fn from_cargo_lock(path: impl AsRef<Path>) -> Result<Self, DocumentationError> {
        let contents = fs::read_to_string(path)?;

        let lock: CargoLock = toml::from_str(&contents)?;

        let mut resolver = Self::new();

        for package in lock.package {
            resolver.register_package_version(package.name, package.version);
        }

        Ok(resolver)
    }

    /// Pins Rust-owned documentation to a specific Rust release.
    ///
    /// Examples include `1.85.0` or `1.98.0`.
    pub fn rust_version(mut self, version: impl Into<String>) -> Self {
        self.rust_version = Some(version.into());

        self
    }

    pub fn set_rust_version(&mut self, version: impl Into<String>) {
        self.rust_version = Some(version.into());
    }

    pub fn configured_rust_version(&self) -> Option<&str> {
        self.rust_version.as_deref()
    }

    pub fn with_package_version(
        mut self,
        package: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        self.register_package_version(package, version);

        self
    }

    pub fn register_package_version(
        &mut self,
        package: impl Into<String>,
        version: impl Into<String>,
    ) {
        self.package_versions
            .entry(package.into())
            .or_default()
            .insert(version.into());
    }

    pub fn package_versions(&self, package: &str) -> Vec<String> {
        self.package_versions
            .get(package)
            .map(|versions| versions.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Resolves docs.rs documentation using the single locked version of the
    /// package.
    ///
    /// Cargo package names containing `-` are converted to `_` for the normal
    /// rustdoc crate identifier.
    pub fn crate_docs(
        &self,
        package: &str,
        path: &str,
    ) -> Result<DocumentationLink, DocumentationError> {
        let rustdoc_crate = package.replace('-', "_");

        self.crate_docs_with_name(package, &rustdoc_crate, path)
    }

    /// Resolves docs.rs documentation when a package has a custom `[lib] name`.
    pub fn crate_docs_with_name(
        &self,
        package: &str,
        rustdoc_crate: &str,
        path: &str,
    ) -> Result<DocumentationLink, DocumentationError> {
        let version = self.unique_package_version(package)?;

        Ok(Self::crate_docs_at_with_name(
            package,
            rustdoc_crate,
            version,
            path,
        ))
    }

    /// Creates an explicitly versioned docs.rs link without consulting the
    /// resolver's package catalog.
    pub fn crate_docs_at(package: &str, version: &str, path: &str) -> DocumentationLink {
        let rustdoc_crate = package.replace('-', "_");

        Self::crate_docs_at_with_name(package, &rustdoc_crate, version, path)
    }

    pub fn crate_docs_at_with_name(
        package: &str,
        rustdoc_crate: &str,
        version: &str,
        path: &str,
    ) -> DocumentationLink {
        DocumentationLink::docs_rs_package(package, rustdoc_crate, version, path)
    }

    pub fn rust_error(&self, code: &str) -> DocumentationLink {
        let code = code.trim().to_ascii_uppercase();

        DocumentationLink::new(
            format!("Rust error {code}"),
            format!("{}/error_codes/{code}.html", self.rust_docs_base()),
        )
        .language("rust")
    }

    pub fn rust_std(&self, path: &str) -> DocumentationLink {
        let path = path.trim_start_matches('/');

        DocumentationLink::new(
            "Rust standard library",
            format!("{}/std/{path}", self.rust_docs_base()),
        )
        .language("rust")
    }

    pub fn rustc(&self, path: &str) -> DocumentationLink {
        let path = path.trim_start_matches('/');

        DocumentationLink::new(
            "rustc documentation",
            format!("{}/rustc/{path}", self.rust_docs_base()),
        )
        .language("rust")
    }

    pub fn rustc_json(&self) -> DocumentationLink {
        self.rustc("json.html")
    }

    pub fn cargo_book(&self, path: &str) -> DocumentationLink {
        let path = path.trim_start_matches('/');

        DocumentationLink::new(
            "Cargo Book",
            format!("{}/cargo/{path}", self.rust_docs_base()),
        )
        .language("toml")
    }

    fn unique_package_version(&self, package: &str) -> Result<&str, DocumentationError> {
        let Some(versions) = self.package_versions.get(package) else {
            return Err(DocumentationError::UnknownPackage {
                package: package.to_owned(),
            });
        };

        if versions.len() != 1 {
            return Err(DocumentationError::AmbiguousPackage {
                package: package.to_owned(),

                versions: versions.iter().cloned().collect(),
            });
        }

        Ok(versions.iter().next().expect("non-empty version set"))
    }

    fn rust_docs_base(&self) -> String {
        match &self.rust_version {
            Some(version) => {
                format!("https://doc.rust-lang.org/{version}")
            }

            None => "https://doc.rust-lang.org".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct CargoLock {
    #[serde(default)]
    package: Vec<LockedPackage>,
}

#[derive(Debug, Deserialize)]
struct LockedPackage {
    name: String,
    version: String,
}
