//! The workspace sandbox for the MCP server.
//!
//! An agent may only touch files inside the workspace directory. Every path it
//! sends is checked here before any file is opened:
//!
//! - `..` components are rejected outright (even when they would stay inside);
//! - absolute paths are accepted only when they point inside the workspace;
//! - symlinks are resolved and the *target* must be inside the workspace;
//! - only `.rpk` files may be written, plus the `.pdf` an agent exports
//!   from the live board (`export_pdf`).

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PathError {
    #[error("path is empty")]
    Empty,
    #[error("path `{0}` contains `..`; use a path inside the workspace without parent references")]
    Traversal(String),
    #[error("path `{0}` is outside the workspace")]
    Outside(String),
    #[error("path `{0}` resolves (through a symlink) to a location outside the workspace")]
    SymlinkEscape(String),
    #[error("only .rpk lesson files may be written, got `{0}`")]
    NotLesson(String),
    #[error("an exported PDF must be saved as a .pdf file, got `{0}`")]
    NotPdf(String),
    #[error("`{0}` does not exist")]
    NotFound(String),
    #[error("`{0}` is not a regular file")]
    NotFile(String),
    #[error("cannot resolve `{path}`: {reason}")]
    Io { path: String, reason: String },
}

/// A canonicalised workspace root that hands out checked paths.
#[derive(Clone, Debug)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    pub fn new(dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let root = dir.as_ref().canonicalize()?;
        if !root.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                format!("{} is not a directory", root.display()),
            ));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path relative to the workspace root, with `/` separators, for display.
    pub fn display(&self, abs: &Path) -> String {
        abs.strip_prefix(&self.root)
            .unwrap_or(abs)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Lexical checks shared by every resolver; returns the joined (not yet
    /// canonicalised) absolute path.
    fn lexical(&self, user: &str) -> Result<PathBuf, PathError> {
        if user.trim().is_empty() {
            return Err(PathError::Empty);
        }
        if user.contains('\0') {
            return Err(PathError::Outside(user.escape_debug().to_string()));
        }
        let p = Path::new(user);
        if p.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(PathError::Traversal(user.to_string()));
        }
        let joined = if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.root.join(p)
        };
        // Absolute paths must already be lexically inside; this catches
        // `/etc/passwd` before we touch the filesystem at all. The real check
        // happens after canonicalisation.
        if p.is_absolute() && !self.lexically_inside(&joined) {
            return Err(PathError::Outside(user.to_string()));
        }
        Ok(joined)
    }

    fn lexically_inside(&self, abs: &Path) -> bool {
        // An absolute path may name the root through a non-canonical spelling
        // (e.g. /tmp vs /private/tmp on macOS), so compare against both.
        abs.starts_with(&self.root)
            || abs
                .canonicalize()
                .map(|c| c.starts_with(&self.root))
                .unwrap_or(false)
            || {
                // Deepest existing ancestor, canonicalised.
                let mut anc = abs.parent();
                let mut ok = false;
                while let Some(a) = anc {
                    if let Ok(c) = a.canonicalize() {
                        ok = c.starts_with(&self.root);
                        break;
                    }
                    anc = a.parent();
                }
                ok
            }
    }

    fn inside(&self, canonical: &Path) -> bool {
        canonical.starts_with(&self.root)
    }

    /// Resolve an existing file for reading.
    pub fn resolve_existing(&self, user: &str) -> Result<PathBuf, PathError> {
        let joined = self.lexical(user)?;
        if std::fs::symlink_metadata(&joined).is_err() {
            return Err(PathError::NotFound(user.to_string()));
        }
        let canonical = joined.canonicalize().map_err(|e| io_err(user, e))?;
        if !self.inside(&canonical) {
            return Err(self.escape_error(user, &joined));
        }
        if !canonical.is_file() {
            return Err(PathError::NotFile(user.to_string()));
        }
        Ok(canonical)
    }

    /// Resolve an existing `.rpk` lesson for reading or in-place update.
    pub fn resolve_lesson(&self, user: &str) -> Result<PathBuf, PathError> {
        let p = self.resolve_existing(user)?;
        if !is_lesson_path(&p) {
            return Err(PathError::NotLesson(user.to_string()));
        }
        Ok(p)
    }

    /// Resolve a `.rpk` path that will be created. Missing parent directories
    /// are allowed (they must themselves end up inside the workspace); the
    /// caller creates them.
    pub fn resolve_new_lesson(&self, user: &str) -> Result<PathBuf, PathError> {
        self.resolve_new(user, is_lesson_path, PathError::NotLesson)
    }

    /// Resolve a `.pdf` path to write (an exported lesson), like
    /// [`Workspace::resolve_new_lesson`].
    pub fn resolve_new_pdf(&self, user: &str) -> Result<PathBuf, PathError> {
        self.resolve_new(user, is_pdf_path, PathError::NotPdf)
    }

    fn resolve_new(
        &self,
        user: &str,
        is_lesson_path: fn(&Path) -> bool,
        bad: fn(String) -> PathError,
    ) -> Result<PathBuf, PathError> {
        let joined = self.lexical(user)?;
        if !is_lesson_path(&joined) {
            return Err(bad(user.to_string()));
        }
        // If something already exists at the path (file or dangling link),
        // resolve it fully so a symlink cannot redirect the write.
        if std::fs::symlink_metadata(&joined).is_ok() {
            let canonical = joined
                .canonicalize()
                .map_err(|_| self.escape_error(user, &joined))?;
            if !self.inside(&canonical) {
                return Err(self.escape_error(user, &joined));
            }
            if !is_lesson_path(&canonical) {
                return Err(bad(user.to_string()));
            }
            return Ok(canonical);
        }
        // Otherwise canonicalise the deepest existing ancestor; the remaining
        // components are plain names (no `..`, checked above).
        let file_name = joined
            .file_name()
            .ok_or_else(|| bad(user.to_string()))?
            .to_owned();
        let mut existing = joined
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.root.clone());
        let mut tail: Vec<std::ffi::OsString> = Vec::new();
        loop {
            match std::fs::symlink_metadata(&existing) {
                Ok(_) => break,
                Err(_) => {
                    let name = existing
                        .file_name()
                        .ok_or_else(|| PathError::Outside(user.to_string()))?
                        .to_owned();
                    tail.push(name);
                    existing = existing
                        .parent()
                        .ok_or_else(|| PathError::Outside(user.to_string()))?
                        .to_path_buf();
                }
            }
        }
        let canonical_parent = existing.canonicalize().map_err(|e| io_err(user, e))?;
        if !self.inside(&canonical_parent) {
            return Err(self.escape_error(user, &existing));
        }
        if !canonical_parent.is_dir() {
            return Err(PathError::Io {
                path: user.to_string(),
                reason: "a parent component is not a directory".into(),
            });
        }
        let mut out = canonical_parent;
        for t in tail.iter().rev() {
            out.push(t);
        }
        out.push(file_name);
        Ok(out)
    }

    fn escape_error(&self, user: &str, joined: &Path) -> PathError {
        // Distinguish a symlink escape from a plain outside path for clearer errors.
        let mut cur = Some(joined);
        while let Some(c) = cur {
            if std::fs::symlink_metadata(c)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
            {
                return PathError::SymlinkEscape(user.to_string());
            }
            if c == self.root {
                break;
            }
            cur = c.parent();
        }
        PathError::Outside(user.to_string())
    }
}

pub fn is_pdf_path(p: &Path) -> bool {
    p.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

pub fn is_lesson_path(p: &Path) -> bool {
    p.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|e| e.eq_ignore_ascii_case("rpk"))
}

fn io_err(user: &str, e: std::io::Error) -> PathError {
    PathError::Io {
        path: user.to_string(),
        reason: e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::new(dir.path()).unwrap();
        (dir, ws)
    }

    #[test]
    fn relative_paths_inside_are_accepted() {
        let (_d, ws) = ws();
        let p = ws.resolve_new_lesson("lessons/week1/intro.rpk").unwrap();
        assert!(p.starts_with(ws.root()));
        assert!(p.ends_with("lessons/week1/intro.rpk"));
        std::fs::write(ws.root().join("a.rpk"), b"x").unwrap();
        assert_eq!(ws.resolve_lesson("a.rpk").unwrap(), ws.root().join("a.rpk"));
        assert_eq!(
            ws.resolve_lesson("./a.rpk").unwrap(),
            ws.root().join("a.rpk")
        );
    }

    #[test]
    fn traversal_is_rejected() {
        let (_d, ws) = ws();
        for p in ["../evil.rpk", "lessons/../../evil.rpk", "a/../b.rpk", ".."] {
            assert!(
                matches!(ws.resolve_new_lesson(p), Err(PathError::Traversal(_))),
                "{p}"
            );
            assert!(
                matches!(ws.resolve_existing(p), Err(PathError::Traversal(_))),
                "{p}"
            );
        }
        assert_eq!(ws.resolve_new_lesson(""), Err(PathError::Empty));
        assert!(ws.resolve_new_lesson("a\0b.rpk").is_err());
    }

    #[test]
    fn absolute_paths_outside_are_rejected_inside_accepted() {
        let (_d, ws) = ws();
        let other = tempfile::tempdir().unwrap();
        let outside = other.path().join("x.rpk");
        assert!(matches!(
            ws.resolve_new_lesson(outside.to_str().unwrap()),
            Err(PathError::Outside(_))
        ));
        std::fs::write(&outside, b"x").unwrap();
        assert!(matches!(
            ws.resolve_existing(outside.to_str().unwrap()),
            Err(PathError::Outside(_))
        ));
        assert!(ws.resolve_existing("/etc/passwd").is_err());

        let inside = ws.root().join("in.rpk");
        assert_eq!(
            ws.resolve_new_lesson(inside.to_str().unwrap()).unwrap(),
            inside
        );
    }

    #[test]
    fn only_lessons_may_be_written() {
        let (_d, ws) = ws();
        assert!(matches!(
            ws.resolve_new_lesson("notes.txt"),
            Err(PathError::NotLesson(_))
        ));
        assert!(matches!(
            ws.resolve_new_lesson(".bashrc"),
            Err(PathError::NotLesson(_))
        ));
        assert!(ws.resolve_new_lesson("Lesson.RPK").is_ok());
        std::fs::write(ws.root().join("x.html"), b"x").unwrap();
        assert!(matches!(
            ws.resolve_lesson("x.html"),
            Err(PathError::NotLesson(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escapes_are_rejected() {
        use std::os::unix::fs::symlink;
        let (_d, ws) = ws();
        let other = tempfile::tempdir().unwrap();
        let secret = other.path().join("secret.rpk");
        std::fs::write(&secret, b"top secret").unwrap();

        // A file symlink pointing out.
        symlink(&secret, ws.root().join("link.rpk")).unwrap();
        assert!(matches!(
            ws.resolve_existing("link.rpk"),
            Err(PathError::SymlinkEscape(_))
        ));
        assert!(matches!(
            ws.resolve_new_lesson("link.rpk"),
            Err(PathError::SymlinkEscape(_))
        ));

        // A directory symlink pointing out, for read and for create.
        symlink(other.path(), ws.root().join("outdir")).unwrap();
        assert!(matches!(
            ws.resolve_existing("outdir/secret.rpk"),
            Err(PathError::SymlinkEscape(_))
        ));
        assert!(matches!(
            ws.resolve_new_lesson("outdir/new.rpk"),
            Err(PathError::SymlinkEscape(_))
        ));
        assert!(matches!(
            ws.resolve_new_lesson("outdir/deeper/new.rpk"),
            Err(PathError::SymlinkEscape(_))
        ));

        // A dangling link pointing out cannot be used to create a file there.
        symlink(
            other.path().join("not-yet.rpk"),
            ws.root().join("dangling.rpk"),
        )
        .unwrap();
        assert!(ws.resolve_new_lesson("dangling.rpk").is_err());
        assert!(!other.path().join("not-yet.rpk").exists());

        // Links that stay inside are fine and resolve to their target.
        std::fs::create_dir(ws.root().join("real")).unwrap();
        std::fs::write(ws.root().join("real/a.rpk"), b"x").unwrap();
        symlink(ws.root().join("real"), ws.root().join("alias")).unwrap();
        assert_eq!(
            ws.resolve_lesson("alias/a.rpk").unwrap(),
            ws.root().join("real/a.rpk")
        );
    }
}
