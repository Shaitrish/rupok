//! The lesson library on disk: one root folder (default `Documents/Rupok`)
//! holding `Subject/Course/Chapter/lesson.rpk`. These are the same plain
//! folders that the `rupok mcp` workspace and sync clients (Drive, OneDrive)
//! use, so the app, AI agents and other devices all see the same files.
//! See `docs/library.md`.
//!
//! The board reaches this through the `library_*` commands
//! (`library_cmd.rs`). Every path is relative to the root and checked here
//! first: no `..`, no absolute paths, no backslashes, and no symlink that
//! leads outside the root. Lessons are validated (`Package::read`) and written
//! atomically (`files::write_atomic`: temporary file, fsync, rename).
//!
//! Nothing here depends on Tauri, so `cargo test --workspace` covers it on
//! any machine.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::files::{self, is_lesson_path};

/// The trash folder inside the root (a dot folder: agents and most tools skip it).
pub const TRASH_DIR: &str = ".trash";
/// The library folder's name inside the user's Documents folder.
pub const DEFAULT_DIR_NAME: &str = "Rupok";
/// Largest file `read` returns whole.
const MAX_READ: u64 = 1 << 30;
/// Largest `read_range` slice.
const MAX_RANGE: u64 = 64 << 20;
/// Largest trash record (`.trash/<id>/trash.json`).
const MAX_TRASH_RECORD: usize = 64 << 10;

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("invalid library path `{0}`")]
    BadPath(String),
    #[error("`{0}` is outside the library folder")]
    Outside(String),
    #[error("`{0}` does not exist")]
    NotFound(String),
    #[error("`{0}` already exists")]
    Exists(String),
    #[error("only .rpk lessons can be written to the library, not `{0}`")]
    NotAllowed(String),
    #[error("a folder cannot be moved or copied into itself")]
    IntoItself,
    #[error("`{0}` is too large")]
    TooLarge(String),
    #[error("not a valid lesson, refusing to write it: {0}")]
    Invalid(#[from] rupok_format::FormatError),
    #[error("{0}")]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, LibraryError>;

/// One file or folder, as the board's `FsDriver` expects it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RawEntry {
    pub name: String,
    pub dir: bool,
    pub size: u64,
    /// Modification time, ms since the Unix epoch.
    pub mtime: f64,
}

/// `<documents>/Rupok`.
pub fn default_root(documents: &Path) -> PathBuf {
    documents.join(DEFAULT_DIR_NAME)
}

fn mtime_ms(m: &fs::Metadata) -> f64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

fn entry(name: String, m: &fs::Metadata) -> RawEntry {
    RawEntry {
        name,
        dir: m.is_dir(),
        size: if m.is_dir() { 0 } else { m.len() },
        mtime: mtime_ms(m),
    }
}

/// True for `rel` = `a` or `a/…`.
fn inside(rel: &str, dir: &str) -> bool {
    dir.is_empty() || rel == dir || rel.starts_with(&format!("{dir}/"))
}

/// A library rooted at a canonical folder.
#[derive(Clone, Debug)]
pub struct Library {
    root: PathBuf,
}

impl Library {
    /// Open the library at `root`, creating the folder if needed.
    pub fn open(root: impl AsRef<Path>) -> io::Result<Self> {
        fs::create_dir_all(root.as_ref())?;
        let root = root.as_ref().canonicalize()?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("{} is not a folder", root.display()),
            ));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Lexical checks; returns the joined path. `""` is the root.
    fn join(&self, rel: &str) -> Result<PathBuf> {
        let bad = || LibraryError::BadPath(rel.escape_debug().to_string());
        if rel.contains('\0') || rel.contains('\\') || rel.starts_with('/') {
            return Err(bad());
        }
        if rel.is_empty() {
            return Ok(self.root.clone());
        }
        for part in rel.split('/') {
            // `:` would be a drive letter or an NTFS stream on Windows.
            if part.is_empty() || part == "." || part == ".." || part.contains(':') {
                return Err(bad());
            }
        }
        Ok(self.root.join(rel))
    }

    /// The path for `rel`, after checking that it (or its deepest existing
    /// ancestor) resolves inside the root, so a symlink cannot lead out.
    fn resolve(&self, rel: &str) -> Result<PathBuf> {
        let p = self.join(rel)?;
        let mut cur = Some(p.as_path());
        while let Some(c) = cur {
            if fs::symlink_metadata(c).is_ok() {
                let canon = c
                    .canonicalize()
                    .map_err(|_| LibraryError::Outside(rel.to_string()))?;
                if !canon.starts_with(&self.root) {
                    return Err(LibraryError::Outside(rel.to_string()));
                }
                break;
            }
            cur = c.parent();
        }
        Ok(p)
    }

    /// Direct children of a folder. Symlinks and names that are not UTF-8 are skipped.
    pub fn list(&self, dir: &str) -> Result<Vec<RawEntry>> {
        let p = self.resolve(dir)?;
        let rd = fs::read_dir(&p).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => LibraryError::NotFound(dir.to_string()),
            _ => e.into(),
        })?;
        let mut out = Vec::new();
        for de in rd.flatten() {
            let Ok(ft) = de.file_type() else { continue };
            if ft.is_symlink() || !(ft.is_dir() || ft.is_file()) {
                continue;
            }
            let Ok(name) = de.file_name().into_string() else {
                continue;
            };
            if let Ok(m) = de.metadata() {
                out.push(entry(name, &m));
            }
        }
        Ok(out)
    }

    pub fn stat(&self, rel: &str) -> Result<Option<RawEntry>> {
        let p = self.resolve(rel)?;
        match fs::metadata(&p) {
            Ok(m) => {
                let name = rel.rsplit('/').next().unwrap_or_default().to_string();
                Ok(Some(entry(name, &m)))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn file(&self, rel: &str) -> Result<(File, u64)> {
        let p = self.resolve(rel)?;
        let m = fs::metadata(&p).map_err(|_| LibraryError::NotFound(rel.to_string()))?;
        if !m.is_file() {
            return Err(LibraryError::NotFound(rel.to_string()));
        }
        Ok((File::open(&p)?, m.len()))
    }

    pub fn read(&self, rel: &str) -> Result<Vec<u8>> {
        let (mut f, len) = self.file(rel)?;
        if len > MAX_READ {
            return Err(LibraryError::TooLarge(rel.to_string()));
        }
        let mut out = Vec::with_capacity(len as usize);
        f.read_to_end(&mut out)?;
        Ok(out)
    }

    /// Bytes `[start, end)` of a file (clamped to its length).
    pub fn read_range(&self, rel: &str, start: u64, end: u64) -> Result<Vec<u8>> {
        let (mut f, len) = self.file(rel)?;
        let end = end.min(len);
        let start = start.min(end);
        if end - start > MAX_RANGE {
            return Err(LibraryError::TooLarge(rel.to_string()));
        }
        f.seek(SeekFrom::Start(start))?;
        let mut out = vec![0; (end - start) as usize];
        f.read_exact(&mut out)?;
        Ok(out)
    }

    /// Write a lesson (validated, atomic). The only other file the library
    /// writes is a trash record, `.trash/<id>/trash.json`.
    pub fn write(&self, rel: &str, bytes: &[u8]) -> Result<()> {
        let p = self.resolve(rel)?;
        let parts: Vec<&str> = rel.split('/').collect();
        let trash_record = parts.len() == 3 && parts[0] == TRASH_DIR && parts[2] == "trash.json";
        if is_lesson_path(&p) {
            rupok_format::Package::read(bytes)?;
        } else if trash_record {
            if bytes.len() > MAX_TRASH_RECORD {
                return Err(LibraryError::TooLarge(rel.to_string()));
            }
        } else {
            return Err(LibraryError::NotAllowed(rel.to_string()));
        }
        if p.is_dir() {
            return Err(LibraryError::Exists(rel.to_string()));
        }
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        files::write_atomic(&p, bytes)?;
        Ok(())
    }

    pub fn mkdir(&self, rel: &str) -> Result<()> {
        let p = self.resolve(rel)?;
        fs::create_dir_all(p)?;
        Ok(())
    }

    fn check_pair(&self, from: &str, to: &str) -> Result<(PathBuf, PathBuf)> {
        if from.is_empty() {
            return Err(LibraryError::BadPath(from.to_string()));
        }
        let src = self.resolve(from)?;
        let dst = self.resolve(to)?;
        if to.is_empty() || inside(to, from) {
            return Err(LibraryError::IntoItself);
        }
        if fs::symlink_metadata(&src).is_err() {
            return Err(LibraryError::NotFound(from.to_string()));
        }
        if fs::symlink_metadata(&dst).is_ok() {
            return Err(LibraryError::Exists(to.to_string()));
        }
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok((src, dst))
    }

    /// Move or rename a file or folder; `to` must not exist.
    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        let (src, dst) = self.check_pair(from, to)?;
        fs::rename(src, dst)?;
        Ok(())
    }

    /// Copy a file or folder (recursively, symlinks skipped); `to` must not exist.
    pub fn copy(&self, from: &str, to: &str) -> Result<()> {
        let (src, dst) = self.check_pair(from, to)?;
        copy_tree(&src, &dst)?;
        Ok(())
    }

    /// Delete a file or folder (recursively). The root itself cannot be removed.
    pub fn remove(&self, rel: &str) -> Result<()> {
        if rel.is_empty() {
            return Err(LibraryError::BadPath(rel.to_string()));
        }
        let p = self.resolve(rel)?;
        match fs::symlink_metadata(&p) {
            Ok(m) if m.is_dir() => fs::remove_dir_all(&p)?,
            Ok(_) => fs::remove_file(&p)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(())
    }
}

fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    let m = fs::symlink_metadata(src)?;
    if m.is_dir() {
        fs::create_dir(dst)?;
        for de in fs::read_dir(src)?.flatten() {
            let ft = de.file_type()?;
            if ft.is_symlink() {
                continue;
            }
            copy_tree(&de.path(), &dst.join(de.file_name()))?;
        }
    } else if m.is_file() {
        fs::copy(src, dst)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lesson_bytes(title: &str) -> Vec<u8> {
        let lesson = rupok_core::Lesson::new(title, 1.0);
        rupok_format::Package::from_lesson(&lesson, "test", 1.0)
            .write()
            .unwrap()
    }

    fn lib() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(dir.path().join("Rupok")).unwrap();
        (dir, lib)
    }

    #[test]
    fn opens_creates_and_defaults_to_documents() {
        let (_d, lib) = lib();
        assert!(lib.root().is_dir());
        assert_eq!(
            default_root(Path::new("/home/t/Documents")),
            Path::new("/home/t/Documents/Rupok")
        );
    }

    #[test]
    fn writes_lists_and_reads_the_course_layout() {
        let (_d, lib) = lib();
        let path = "ICT/Grade 9 ICT/01 Number systems/Binary numbers.rpk";
        let bytes = lesson_bytes("Binary numbers");
        lib.write(path, &bytes).unwrap();
        assert_eq!(lib.read(path).unwrap(), bytes);
        let top = lib.list("").unwrap();
        assert_eq!(top.len(), 1);
        assert!(top[0].dir && top[0].name == "ICT");
        let ch = lib.list("ICT/Grade 9 ICT/01 Number systems").unwrap();
        assert_eq!(ch.len(), 1);
        assert_eq!(ch[0].name, "Binary numbers.rpk");
        assert_eq!(ch[0].size, bytes.len() as u64);
        assert!(ch[0].mtime > 0.0);
        let st = lib.stat(path).unwrap().unwrap();
        assert_eq!(st.size, bytes.len() as u64);
        assert!(lib.stat("ICT/missing.rpk").unwrap().is_none());
        assert_eq!(lib.read_range(path, 0, 4).unwrap(), b"PK\x03\x04");
        assert_eq!(
            lib.read_range(path, bytes.len() as u64 - 2, u64::MAX)
                .unwrap()
                .len(),
            2
        );
        assert!(!files::temp_path(&lib.root().join(path)).exists());
    }

    #[test]
    fn refuses_paths_that_leave_the_library() {
        let (_d, lib) = lib();
        let bytes = lesson_bytes("x");
        for p in [
            "../x.rpk",
            "a/../../x.rpk",
            "/etc/x.rpk",
            "a\\b.rpk",
            "a//b.rpk",
            "./x.rpk",
            "a\0b.rpk",
            "C:x.rpk",
        ] {
            assert!(
                matches!(lib.write(p, &bytes), Err(LibraryError::BadPath(_))),
                "{p}"
            );
            assert!(lib.list(p).is_err(), "{p}");
        }
        assert!(lib.remove("").is_err(), "the root cannot be removed");
    }

    #[test]
    fn only_valid_lessons_and_trash_records_are_written() {
        let (_d, lib) = lib();
        assert!(matches!(
            lib.write("notes.txt", b"hi"),
            Err(LibraryError::NotAllowed(_))
        ));
        assert!(matches!(
            lib.write("a/trash.json", b"{}"),
            Err(LibraryError::NotAllowed(_))
        ));
        assert!(matches!(
            lib.write("bad.rpk", b"not a zip"),
            Err(LibraryError::Invalid(_))
        ));
        assert!(lib.stat("bad.rpk").unwrap().is_none());
        lib.write(".trash/123-abc/trash.json", br#"{"from":"a.rpk"}"#)
            .unwrap();
        assert!(matches!(
            lib.write(".trash/1/trash.json", &vec![b' '; MAX_TRASH_RECORD + 1]),
            Err(LibraryError::TooLarge(_))
        ));
        // A good lesson is never replaced by garbage.
        let good = lesson_bytes("good");
        lib.write("good.rpk", &good).unwrap();
        assert!(lib.write("good.rpk", b"garbage").is_err());
        assert_eq!(lib.read("good.rpk").unwrap(), good);
    }

    #[test]
    fn rename_copy_and_remove() {
        let (_d, lib) = lib();
        lib.write("A/one.rpk", &lesson_bytes("one")).unwrap();
        lib.write("B/two.rpk", &lesson_bytes("two")).unwrap();
        assert!(matches!(
            lib.rename("A/one.rpk", "B/two.rpk"),
            Err(LibraryError::Exists(_))
        ));
        assert!(matches!(
            lib.rename("A", "A/inner"),
            Err(LibraryError::IntoItself)
        ));
        assert!(matches!(
            lib.rename("missing.rpk", "x.rpk"),
            Err(LibraryError::NotFound(_))
        ));
        lib.rename("A/one.rpk", "B/Sub/one.rpk").unwrap();
        assert!(lib.stat("A/one.rpk").unwrap().is_none());
        assert!(lib.stat("B/Sub/one.rpk").unwrap().is_some());
        lib.copy("B", "C").unwrap();
        assert!(lib.stat("C/Sub/one.rpk").unwrap().is_some());
        assert!(lib.stat("C/two.rpk").unwrap().is_some());
        assert!(matches!(
            lib.copy("B", "B/x"),
            Err(LibraryError::IntoItself)
        ));
        lib.remove("C").unwrap();
        assert!(lib.stat("C").unwrap().is_none());
        lib.remove("C").unwrap(); // already gone: fine
        lib.mkdir("Empty/Deeper").unwrap();
        assert!(lib.stat("Empty/Deeper").unwrap().unwrap().dir);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_lead_out() {
        use std::os::unix::fs::symlink;
        let (_d, lib) = lib();
        let other = tempfile::tempdir().unwrap();
        fs::write(other.path().join("secret.rpk"), lesson_bytes("secret")).unwrap();
        symlink(other.path(), lib.root().join("out")).unwrap();
        assert!(matches!(
            lib.read("out/secret.rpk"),
            Err(LibraryError::Outside(_))
        ));
        assert!(matches!(
            lib.write("out/new.rpk", &lesson_bytes("x")),
            Err(LibraryError::Outside(_))
        ));
        assert!(matches!(
            lib.remove("out/secret.rpk"),
            Err(LibraryError::Outside(_))
        ));
        assert!(other.path().join("secret.rpk").exists());
        // Listing skips symlinks altogether.
        assert!(lib.list("").unwrap().iter().all(|e| e.name != "out"));
    }
}
