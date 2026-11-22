//! The Smart Object authoring spec handed to agents by `read_object_spec`.
//!
//! The authoritative text is `spec/smart-object.md` in the rupok repo. When
//! the server runs somewhere that file can be found (walking up from the
//! workspace) it is served verbatim; otherwise the concise summary below is.

use std::path::{Path, PathBuf};

/// Look for `spec/smart-object.md` in `start` and each of its ancestors.
pub fn find_object_spec(start: &Path) -> Option<(PathBuf, String)> {
    start.ancestors().find_map(|dir| {
        let p = dir.join("spec").join("smart-object.md");
        let text = std::fs::read_to_string(&p).ok()?;
        (!text.trim().is_empty()).then_some((p, text))
    })
}

/// The spec text and where it came from (`None` = embedded fallback).
pub fn object_spec(start: &Path) -> (Option<PathBuf>, String) {
    match find_object_spec(start) {
        Some((p, text)) => (Some(p), text),
        None => (None, fallback_spec()),
    }
}

pub fn fallback_spec() -> String {
    FALLBACK_SPEC.replace("{LIBS}", &rupok_format::LIBS.join(", "))
}

const FALLBACK_SPEC: &str = r#"# rupok Smart Objects — authoring summary

(Embedded summary. The full spec is `spec/smart-object.md` in the rupok repository.)

A Smart Object is a small, self-contained web page that runs live on a rupok
board or slide. Teachers present them on classroom projectors and interactive
touch boards, often with no internet connection.

## Shape of an object
- One HTML document (the bundle's `index.html`). Inline your CSS and JS.
- It renders inside a sandboxed iframe whose viewport is exactly the object's
  size (`width` × `height` in CSS px, default 1280×720). Lay out with
  `100vw`/`100vh`, flexbox or grid so it fills that box; never assume a
  scrolling page.
- Slides are 1920×1080. Pass `width: 1920, height: 1080` for a full-slide
  object, or a smaller size and an `x`/`y` position to share the slide with
  text. Omitted `x`/`y` centre the object on the slide.

## Design rules for classrooms
- Text at 24px or larger (body 28–32px, headings 48px+). It is read from the
  back of the room.
- High contrast, large touch targets (at least 48px), no hover-only controls:
  the board is driven by fingers and pens.
- No tiny detail that only works on a laptop screen. One idea per object.

## Offline by default
- Network access is denied. Any remote resource — `<script src>`,
  `<link href>` stylesheets, `<img src>`, CSS `url()`, `import`, `fetch()`,
  XHR, WebSocket on `http(s)://` or `//` URLs — is a validation **error**.
  Plain `<a href>` links are allowed.
- Use the vendored libraries through `rupoklib://<library>/<file>` instead of
  CDNs, e.g. `import * as THREE from "rupoklib://three/three.module.js"`.
  Available libraries: {LIBS}.
- If live data is essential, list each plain hostname in `network_hosts`
  (e.g. `["api.open-meteo.com"]`; subdomains are included). No schemes,
  paths, ports or wildcards. Prefer embedding the data.

## Sandbox limits
- No access to `window.top`, `parent.document` or `top.location`.
- No `localStorage`, `sessionStorage` or `indexedDB`. Persist state with
  `rupok.state.save(jsonString)`; the board stores it with the lesson.

## The `rupok` runtime
- A global `rupok` object is injected into every object.
- Build steps: declare `steps: N` (N > 0) to let the presenter click through
  N steps inside the object with the slide clicker. Subscribe with
  `rupok.steps.onChange(step => { ... })`; `step` runs from 0 (initial
  state) to N. Render each step idempotently from the number alone — the
  presenter can jump backwards. Objects with `steps: 0` are static.
- `rupok.state.save(json)` stores an opaque JSON string for the object.

## Validation
- Call `validate_smart_object` (or just `add_smart_object`, which validates
  first). Errors block placement and nothing is saved; fix the HTML and retry.
  Warnings (unknown library, storage use, declared steps never used) are
  advice.
- Limits: title required (≤ 200 chars), size 50–8000 px per side, steps ≤ 500,
  bundle ≤ 25 MiB.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_spec_in_an_ancestor_and_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();
        // Nothing yet: a temp dir has no spec above it (unless /tmp/spec exists,
        // which we do not control, so only assert the fallback content shape).
        let fb = fallback_spec();
        assert!(fb.contains("rupoklib://") && fb.contains("three") && !fb.contains("{LIBS}"));

        std::fs::create_dir_all(dir.path().join("a/spec")).unwrap();
        std::fs::write(dir.path().join("a/spec/smart-object.md"), "# Real spec").unwrap();
        let (src, text) = object_spec(&deep);
        assert_eq!(text, "# Real spec");
        assert_eq!(src.unwrap(), dir.path().join("a/spec/smart-object.md"));
    }
}
