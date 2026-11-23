//! The `.rpk` lesson package (spec: `spec/rupok-format.md`) and Smart Object
//! bundles (spec: `spec/smart-object.md`).
//!
//! ```text
//! mimetype             "application/vnd.rupok.lesson+zip" (first, stored)
//! manifest.json        title, format version, course refs
//! README.md            what this file is + readable outline (derived)
//! content.md           readable outline of the lesson (derived)
//! content.json         the same outline, structured (derived)
//! doc.loro             Loro snapshot with full history (= the recording)
//! assets/<blake3>.ext  content-addressed media
//! objects/<id>/...     Smart Object bundles
//! thumbs/<n>.png       page thumbnails
//! layers/<peer>.loro   optional student annotation layers
//! ```

pub mod object;
pub mod package;
pub mod readable;

pub use object::{Diagnostic, LIBS, ObjectBundle, ObjectManifest, Severity};
pub use package::{CourseRef, Manifest, Package};
pub use readable::{Content, Outline, ReadableCache};

pub const MIMETYPE: &str = "application/vnd.rupok.lesson+zip";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("not a rupok package: {0}")]
    NotAPackage(String),
    #[error("package format version {0} is newer than this build supports ({FORMAT_VERSION})")]
    TooNew(u32),
    #[error("unsafe path in package: {0}")]
    UnsafePath(String),
    #[error("package limit exceeded: {0}")]
    Limit(String),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("document: {0}")]
    Core(#[from] rupok_core::CoreError),
    #[error("invalid smart object: {0}")]
    InvalidObject(String),
}

pub type Result<T> = std::result::Result<T, FormatError>;

/// Reject absolute paths, `..`, backslashes and control characters.
pub fn safe_path(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 512
        && !p.starts_with('/')
        && !p.contains('\\')
        && !p.chars().any(|c| c.is_control())
        && p.split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

#[cfg(test)]
mod tests {
    #[test]
    fn safe_paths() {
        assert!(super::safe_path("objects/a/index.html"));
        assert!(!super::safe_path("../etc/passwd"));
        assert!(!super::safe_path("/abs"));
        assert!(!super::safe_path("a//b"));
        assert!(!super::safe_path("a\\b"));
        assert!(!super::safe_path("a/./b"));
    }
}
