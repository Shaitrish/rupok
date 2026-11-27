//! The board speed test's report (apps/board/src/speedTest.ts): written to
//! `Documents/Rupok/speed-test-<date>.txt`, so a teacher who forgets to copy
//! it still has it to send.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, Runtime};

use crate::library::DEFAULT_DIR_NAME;

/// Reports are a page of text; anything bigger is not one.
const MAX_REPORT: usize = 256 * 1024;

/// `stamp`: the date and time in the file's name, digits and dashes only.
#[tauri::command]
pub fn save_speed_report<R: Runtime>(
    app: AppHandle<R>,
    stamp: String,
    text: String,
) -> Result<String, String> {
    if stamp.is_empty()
        || stamp.len() > 24
        || !stamp.chars().all(|c| c.is_ascii_digit() || c == '-')
    {
        return Err("bad report name".into());
    }
    if text.len() > MAX_REPORT {
        return Err("the report is too large".into());
    }
    let dir: PathBuf = app
        .path()
        .document_dir()
        .or_else(|_| app.path().home_dir())
        .map_err(|e| e.to_string())?
        .join(DEFAULT_DIR_NAME);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("speed-test-{stamp}.txt"));
    // Windows Notepad reads UTF-8 with a byte-order mark as UTF-8.
    let mut bytes = Vec::with_capacity(text.len() + 3);
    bytes.extend_from_slice(b"\xEF\xBB\xBF");
    bytes.extend_from_slice(text.replace('\n', "\r\n").as_bytes());
    crate::files::write_atomic(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}
