//! Lesson files on disk: crash-safe saving, and the paths the OS hands us
//! (double-clicked `.rpk` files, "Open with", a second launch).
//!
//! Nothing here depends on Tauri, so it is unit-tested by
//! `cargo test --workspace` on any machine.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// The lesson file extension (without the dot).
pub const LESSON_EXT: &str = "rpk";

/// Extensions the board imports (`importFiles` in `apps/board/src/controller.ts`).
/// Only these are accepted from the command line and added to the file scope.
pub const OPENABLE_EXTENSIONS: &[&str] = &[
    LESSON_EXT, "pdf", "html", "htm", "png", "jpg", "jpeg", "gif", "webp", "svg",
];

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("lesson files must end in .{LESSON_EXT}")]
    Extension,
    #[error("exported PDFs must end in .pdf")]
    PdfExtension,
    #[error("not a PDF, refusing to write it")]
    NotPdf,
    #[error("not a valid lesson, refusing to overwrite: {0}")]
    Invalid(#[from] rupok_format::FormatError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

/// True when `path` ends in `.rpk` (case-insensitive).
pub fn is_lesson_path(path: &Path) -> bool {
    has_extension(path, &[LESSON_EXT])
}

fn has_extension(path: &Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// The temporary file a save writes first: `<name>.rpk.tmp` next to the
/// target, so the final rename never crosses a file system.
pub fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Save lesson bytes to `path` without ever leaving a truncated file behind.
///
/// 1. The bytes must parse as a package (`Package::read`), so a bug in the UI
///    can never replace a good lesson with garbage.
/// 2. They are written to `<name>.rpk.tmp` in the same directory and synced.
/// 3. The temporary file is renamed over the target (atomic on the same file
///    system on Windows and Linux). The original is never truncated first.
pub fn save_lesson(path: &Path, bytes: &[u8]) -> Result<(), FileError> {
    if !is_lesson_path(path) {
        return Err(FileError::Extension);
    }
    rupok_format::Package::read(bytes)?;
    write_atomic(path, bytes)?;
    Ok(())
}

/// True when `path` ends in `.pdf` (case-insensitive).
pub fn is_pdf_path(path: &Path) -> bool {
    has_extension(path, &["pdf"])
}

/// Save an exported PDF (the board's "Export PDF") to `path`: the target
/// must end in `.pdf` and the bytes must start with the PDF header, then
/// they are written like a lesson (temporary file, sync, rename).
pub fn save_pdf(path: &Path, bytes: &[u8]) -> Result<(), FileError> {
    if !is_pdf_path(path) {
        return Err(FileError::PdfExtension);
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err(FileError::NotPdf);
    }
    write_atomic(path, bytes)?;
    Ok(())
}

/// Write `bytes` to `path` through a synced temporary file and a rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = temp_path(path);
    let result = (|| {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)?;
        sync_parent(path);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Persist the rename itself (directory entry) on Unix. Best effort.
fn sync_parent(path: &Path) {
    #[cfg(unix)]
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty())
        && let Ok(d) = File::open(dir)
    {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Files to open from a command line (`argv` without the program name):
/// existing files with an extension the board imports, resolved against
/// `cwd`. Flags (`-…`) and anything else are ignored.
pub fn paths_from_args<S: AsRef<str>>(args: &[S], cwd: &Path) -> Vec<PathBuf> {
    openable_paths(
        args.iter()
            .map(AsRef::as_ref)
            .filter(|a| !a.is_empty() && !a.starts_with('-'))
            .map(|a| {
                let p = PathBuf::from(a);
                if p.is_absolute() { p } else { cwd.join(p) }
            }),
    )
}

/// The paths the board can open: existing files with an extension it
/// imports. Used for the command line and for the files macOS hands over
/// (Finder double-click, "Open With", drag onto the Dock icon).
pub fn openable_paths(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter(|p| has_extension(p, OPENABLE_EXTENSIONS) && p.is_file())
        .collect()
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

    #[test]
    fn saves_and_replaces_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Fractions.rpk");
        save_lesson(&path, &lesson_bytes("one")).unwrap();
        let second = lesson_bytes("two");
        save_lesson(&path, &second).unwrap();
        assert_eq!(fs::read(&path).unwrap(), second);
        assert!(!temp_path(&path).exists(), "temporary file is cleaned up");
    }

    #[test]
    fn refuses_corrupt_bytes_and_keeps_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lesson.rpk");
        let good = lesson_bytes("good");
        save_lesson(&path, &good).unwrap();
        assert!(matches!(
            save_lesson(&path, b"not a zip"),
            Err(FileError::Invalid(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), good);
        assert!(!temp_path(&path).exists());
    }

    #[test]
    fn failed_write_leaves_the_original_intact() {
        // The temporary path is occupied by a directory, so creating the temp
        // file fails, as a full disk or a crash before the rename would.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lesson.rpk");
        let good = lesson_bytes("good");
        save_lesson(&path, &good).unwrap();
        fs::create_dir(temp_path(&path)).unwrap();
        assert!(save_lesson(&path, &lesson_bytes("new")).is_err());
        assert_eq!(fs::read(&path).unwrap(), good);
    }

    #[test]
    fn only_rpk_targets() {
        let dir = tempfile::tempdir().unwrap();
        let err = save_lesson(&dir.path().join("notes.txt"), &lesson_bytes("x"));
        assert!(matches!(err, Err(FileError::Extension)));
        assert!(is_lesson_path(Path::new("A.RPK")));
        assert!(!is_lesson_path(Path::new("a.rpk.tmp")));
    }

    #[test]
    fn saves_pdfs_and_refuses_anything_else() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Fractions.pdf");
        let pdf = b"%PDF-1.7\n%\xe2\xe3\n1 0 obj<<>>endobj\n%%EOF";
        save_pdf(&path, pdf).unwrap();
        assert_eq!(fs::read(&path).unwrap(), pdf);
        assert!(!temp_path(&path).exists());
        assert!(matches!(save_pdf(&path, b"<html>"), Err(FileError::NotPdf)));
        assert_eq!(fs::read(&path).unwrap(), pdf, "the old file is kept");
        let lesson = dir.path().join("lesson.rpk");
        assert!(matches!(
            save_pdf(&lesson, pdf),
            Err(FileError::PdfExtension)
        ));
        assert!(!lesson.exists());
        assert!(is_pdf_path(Path::new("A.PDF")));
    }

    #[test]
    fn temp_path_is_a_sibling() {
        let p = Path::new("/lessons/maths.rpk");
        assert_eq!(temp_path(p), Path::new("/lessons/maths.rpk.tmp"));
    }

    #[test]
    fn args_keep_existing_importable_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rpk"), b"x").unwrap();
        fs::write(dir.path().join("slides.PDF"), b"x").unwrap();
        fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        let abs = dir.path().join("a.rpk");
        let args = [
            "--flag",
            "a.rpk",
            "slides.PDF",
            "notes.txt",
            "missing.rpk",
            abs.to_str().unwrap(),
        ];
        let got = paths_from_args(&args, dir.path());
        assert_eq!(got, vec![abs.clone(), dir.path().join("slides.PDF"), abs]);
    }

    #[test]
    fn opened_files_keep_existing_importable_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("lesson.RPK"), b"x").unwrap();
        fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        fs::create_dir(dir.path().join("folder.rpk")).unwrap();
        let got = openable_paths([
            dir.path().join("lesson.RPK"),
            dir.path().join("notes.txt"),
            dir.path().join("folder.rpk"),
            dir.path().join("missing.rpk"),
        ]);
        assert_eq!(got, vec![dir.path().join("lesson.RPK")]);
    }
}
