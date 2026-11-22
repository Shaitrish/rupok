//! Smart Object bundles: live, AI-generated HTML content placed on a page.
//!
//! A bundle is `manifest.json` + an entry HTML file + optional assets. It runs
//! in a sandboxed iframe with no network unless the manifest asks for hosts.
//! Common libraries are vendored and referenced as `rupoklib://<name>/<file>` so
//! lessons work offline in a classroom.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{FormatError, Result, safe_path};

pub const OBJECT_SCHEMA: u32 = 1;
const MAX_BUNDLE_BYTES: usize = 25 << 20;

/// Libraries available offline through `rupoklib://`. Keep in sync with
/// `spec/smart-object.md` and `scripts/vendor-libs.mjs`.
pub const LIBS: &[&str] = &[
    "three",
    "d3",
    "katex",
    "mermaid",
    "chart.js",
    "gsap",
    "p5",
    "matter-js",
    "animejs",
    "tailwind",
    "lucide",
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Permissions {
    /// Hosts the object may load from or connect to, e.g. `["api.open-meteo.com"]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub network: Vec<String>,
}

fn default_entry() -> String {
    "index.html".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectManifest {
    pub schema: u32,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default = "default_entry")]
    pub entry: String,
    pub size: Size,
    /// Number of build steps the presenter can click through (0 = static).
    #[serde(default)]
    pub steps: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libs: Vec<String>,
    #[serde(default)]
    pub permissions: Permissions,
    /// Theme tokens the object reads (e.g. `"color.accent"`), for brand kits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub theme: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectBundle {
    pub manifest: ObjectManifest,
    /// Relative path → bytes. Includes the entry file; `manifest.json` is
    /// serialised from `manifest` on write.
    pub files: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
}

impl Diagnostic {
    fn err(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
        }
    }
    fn warn(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message: message.into(),
        }
    }
}

impl ObjectBundle {
    pub fn from_files(mut files: BTreeMap<String, Vec<u8>>) -> Result<Self> {
        let raw = files
            .remove("manifest.json")
            .ok_or_else(|| FormatError::InvalidObject("missing manifest.json".into()))?;
        let manifest: ObjectManifest = serde_json::from_slice(&raw)
            .map_err(|e| FormatError::InvalidObject(format!("manifest.json: {e}")))?;
        for p in files.keys() {
            if !safe_path(p) {
                return Err(FormatError::UnsafePath(p.clone()));
            }
        }
        Ok(Self { manifest, files })
    }

    /// Wrap a single self-contained HTML document (the common agent output).
    pub fn from_single_html(html: &str, title: &str, w: f64, h: f64, steps: u32) -> Self {
        let mut files = BTreeMap::new();
        files.insert("index.html".to_string(), html.as_bytes().to_vec());
        Self {
            manifest: ObjectManifest {
                schema: OBJECT_SCHEMA,
                title: title.to_string(),
                description: String::new(),
                entry: "index.html".into(),
                size: Size { w, h },
                steps,
                libs: detect_libs(html),
                permissions: Permissions::default(),
                theme: Vec::new(),
                generator: None,
            },
            files,
        }
    }

    /// Load a bundle from a directory on disk (CLI / desktop).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_dir(dir: &std::path::Path) -> Result<Self> {
        fn walk(
            base: &std::path::Path,
            dir: &std::path::Path,
            out: &mut BTreeMap<String, Vec<u8>>,
        ) -> Result<()> {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                let ft = entry.file_type()?;
                if ft.is_symlink() {
                    continue; // never follow links out of the bundle
                }
                if ft.is_dir() {
                    walk(base, &path, out)?;
                } else {
                    let rel = path
                        .strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.insert(rel, std::fs::read(&path)?);
                }
            }
            Ok(())
        }
        let mut files = BTreeMap::new();
        walk(dir, dir, &mut files)?;
        Self::from_files(files)
    }

    pub fn entry_html(&self) -> Option<&str> {
        self.files
            .get(&self.manifest.entry)
            .and_then(|b| std::str::from_utf8(b).ok())
    }

    pub fn total_bytes(&self) -> usize {
        self.files.values().map(Vec::len).sum()
    }

    /// Check a bundle against `spec/smart-object.md`. Errors block placement;
    /// warnings are shown to the teacher/agent.
    pub fn validate(&self) -> Vec<Diagnostic> {
        let m = &self.manifest;
        let mut d = Vec::new();
        if m.schema != OBJECT_SCHEMA {
            d.push(Diagnostic::err(
                "schema",
                format!("schema must be {OBJECT_SCHEMA}, got {}", m.schema),
            ));
        }
        if m.title.trim().is_empty() {
            d.push(Diagnostic::err("title", "title is required"));
        } else if m.title.chars().count() > 200 {
            d.push(Diagnostic::warn(
                "title",
                "title is longer than 200 characters",
            ));
        }
        let size_ok = |v: f64| v.is_finite() && (50.0..=8000.0).contains(&v);
        if !size_ok(m.size.w) || !size_ok(m.size.h) {
            d.push(Diagnostic::err(
                "size",
                "size.w and size.h must be between 50 and 8000",
            ));
        }
        if m.steps > 500 {
            d.push(Diagnostic::err("steps", "steps must be ≤ 500"));
        }
        for p in self.files.keys() {
            if !safe_path(p) {
                d.push(Diagnostic::err("path", format!("unsafe path: {p}")));
            }
        }
        if self.total_bytes() > MAX_BUNDLE_BYTES {
            d.push(Diagnostic::err(
                "size_bytes",
                format!(
                    "bundle is {} bytes (max {MAX_BUNDLE_BYTES})",
                    self.total_bytes()
                ),
            ));
        }
        for host in &m.permissions.network {
            if host.contains('/') || host.contains(':') || host.contains('*') || !host.contains('.')
            {
                d.push(Diagnostic::err(
                    "permissions",
                    format!("network entries must be plain hostnames: {host}"),
                ));
            }
        }
        let Some(html) = self.entry_html() else {
            d.push(Diagnostic::err(
                "entry",
                format!("entry file `{}` is missing or not UTF-8", m.entry),
            ));
            return d;
        };
        if !m.entry.ends_with(".html") {
            d.push(Diagnostic::err("entry", "entry must be an .html file"));
        }

        // Scan entry HTML and any JS/CSS files for remote resources.
        let mut sources: Vec<(&str, &str)> = vec![(m.entry.as_str(), html)];
        for (p, b) in &self.files {
            if (p.ends_with(".js") || p.ends_with(".mjs") || p.ends_with(".css"))
                && p != &m.entry
                && let Ok(s) = std::str::from_utf8(b)
            {
                sources.push((p.as_str(), s));
            }
        }
        for (file, src) in sources {
            for r in remote_resources(src) {
                if !m
                    .permissions
                    .network
                    .iter()
                    .any(|h| host_matches(&r.host, h))
                {
                    d.push(Diagnostic::err(
                        "network",
                        format!(
                            "{file}: loads `{}` from {} but network access is not permitted. \
                             Use a rupoklib:// library or inline it, or add the host to permissions.network",
                            r.url, r.host
                        ),
                    ));
                }
            }
            for lib in lib_refs(src) {
                if !LIBS.contains(&lib.as_str()) {
                    d.push(Diagnostic::warn(
                        "lib",
                        format!(
                            "{file}: unknown rupoklib library `{lib}` (known: {})",
                            LIBS.join(", ")
                        ),
                    ));
                }
            }
            if src.contains("window.top")
                || src.contains("parent.document")
                || src.contains("top.location")
            {
                d.push(Diagnostic::warn(
                    "sandbox",
                    format!("{file}: accesses the parent window, which is blocked in the sandbox"),
                ));
            }
            if src.contains("localStorage") || src.contains("indexedDB") {
                d.push(Diagnostic::warn(
                    "storage",
                    format!("{file}: browser storage is unavailable in the sandbox; use rupok.state.save()"),
                ));
            }
        }
        if m.steps > 0
            && !html.contains("rupok")
            && !self
                .files
                .iter()
                .any(|(p, b)| p.ends_with(".js") && memchr(b, b"rupok"))
        {
            d.push(Diagnostic::warn(
                "steps",
                "manifest declares steps but the object never uses rupok.steps",
            ));
        }
        d
    }

    pub fn is_valid(&self) -> bool {
        !self
            .validate()
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

fn memchr(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn host_matches(host: &str, allowed: &str) -> bool {
    host == allowed || host.ends_with(&format!(".{allowed}"))
}

struct Remote {
    url: String,
    host: String,
}

/// Find http(s) URLs used as *resources* (scripts, styles, images, fetches,
/// imports). Plain `<a href>` links are navigation, not loads, and are ignored.
fn remote_resources(src: &str) -> Vec<Remote> {
    let mut out = Vec::new();
    let lower = src.to_ascii_lowercase();
    for scheme in ["https://", "http://", "//"] {
        let mut from = 0;
        while let Some(pos) = lower[from..].find(scheme) {
            let start = from + pos;
            from = start + scheme.len();
            if scheme == "//"
                && (start > 0
                    && (lower.as_bytes()[start - 1] == b':' || lower.as_bytes()[start - 1] == b'/'))
            {
                continue; // part of http(s):// already handled, or a comment line
            }
            // Byte window before the URL, snapped to a char boundary: the
            // source may contain Bangla or any other non-ASCII text.
            let mut ctx_start = start.saturating_sub(48);
            while !lower.is_char_boundary(ctx_start) {
                ctx_start -= 1;
            }
            let ctx = &lower[ctx_start..start];
            // Protocol-relative URLs only count right after src=/href=/url(.
            let is_attr_like = |c: &str| {
                c.trim_end().ends_with("src=\"")
                    || c.trim_end().ends_with("src='")
                    || c.trim_end().ends_with("href=\"")
                    || c.trim_end().ends_with("href='")
                    || c.trim_end().ends_with("url(")
                    || c.trim_end().ends_with("url(\"")
                    || c.trim_end().ends_with("url('")
            };
            if scheme == "//" && !is_attr_like(ctx) {
                continue;
            }
            // A URL counts as a *load* only in code-like positions: an attribute
            // value (src=, <link href=), a CSS url(), or a quoted string passed to
            // import/fetch/WebSocket/XHR. Prose such as "data from https://…" or a
            // plain <a href> link is not a resource load.
            let quoted =
                start > 0 && matches!(lower.as_bytes()[start - 1], b'"' | b'\'' | b'`' | b'(');
            let tail = ctx.trim_end_matches(['"', '\'', '`', '(']).trim_end();
            let attr = ctx.contains("src=")
                && quoted
                && (tail.ends_with("src=") || tail.ends_with("srcset="));
            let link = quoted && tail.ends_with("href=") && ctx.contains("<link");
            let css = tail.ends_with("url") || tail.ends_with("@import");
            let code = quoted
                && (tail.ends_with("import")
                    || tail.ends_with("from")
                    || tail.ends_with("fetch")
                    || tail.ends_with("websocket")
                    || tail.ends_with("open(\"get\",")
                    || tail.ends_with("open('get',")
                    || tail.ends_with("import("));
            let resource = attr || link || css || code;
            let anchor = ctx.contains("<a ") && ctx.contains("href=");
            if !resource || anchor {
                continue;
            }
            let url_end = src[start..]
                .find(|c: char| {
                    c == '"'
                        || c == '\''
                        || c == ')'
                        || c == '`'
                        || c.is_whitespace()
                        || c == '>'
                        || c == '<'
                })
                .map(|e| start + e)
                .unwrap_or(src.len());
            let url = &src[start..url_end];
            let after = url
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .trim_start_matches("//");
            let host = after
                .split(['/', '?', '#', ':'])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            if host.is_empty() || host == "localhost" {
                continue;
            }
            out.push(Remote {
                url: url.to_string(),
                host,
            });
        }
    }
    out
}

fn lib_refs(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = src[from..].find("rupoklib://") {
        let start = from + pos + "rupoklib://".len();
        from = start;
        let name: String = src[start..]
            .chars()
            .take_while(|c| *c != '/' && *c != '"' && *c != '\'' && !c.is_whitespace())
            .collect();
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

fn detect_libs(html: &str) -> Vec<String> {
    lib_refs(html)
        .into_iter()
        .filter(|l| LIBS.contains(&l.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(b: &ObjectBundle) -> Vec<(&'static str, Severity)> {
        b.validate()
            .into_iter()
            .map(|d| (d.code, d.severity))
            .collect()
    }

    #[test]
    fn offline_object_is_valid() {
        let html = r#"<!doctype html><script type="module">
            import * as THREE from "rupoklib://three/three.module.js";
            rupok.steps.onChange(s => {});
        </script><a href="https://en.wikipedia.org/wiki/ENIAC">ENIAC</a>"#;
        let b = ObjectBundle::from_single_html(html, "ENIAC 3D", 960.0, 540.0, 4);
        assert!(b.is_valid(), "{:?}", b.validate());
        assert_eq!(b.manifest.libs, vec!["three".to_string()]);
    }

    #[test]
    fn cdn_script_without_permission_is_an_error() {
        let html = r#"<script src="https://cdn.jsdelivr.net/npm/chart.js"></script>"#;
        let b = ObjectBundle::from_single_html(html, "Chart", 800.0, 400.0, 0);
        assert!(codes(&b).contains(&("network", Severity::Error)));
        let mut b2 = b.clone();
        b2.manifest.permissions.network.push("jsdelivr.net".into());
        assert!(b2.is_valid(), "{:?}", b2.validate());
    }

    #[test]
    fn fetch_and_protocol_relative_are_detected() {
        let b = ObjectBundle::from_single_html(
            r#"<img src="//evil.example/x.png"><script>fetch("https://api.example.com/q")</script>"#,
            "x",
            100.0,
            100.0,
            0,
        );
        let errs: Vec<_> = b
            .validate()
            .into_iter()
            .filter(|d| d.code == "network")
            .collect();
        assert_eq!(errs.len(), 2, "{errs:?}");
    }

    #[test]
    fn structural_problems() {
        let mut b = ObjectBundle::from_single_html("<p>hi</p>", "", 10.0, 100.0, 3);
        b.manifest
            .permissions
            .network
            .push("https://x.com/path".into());
        let c = codes(&b);
        assert!(c.contains(&("title", Severity::Error)));
        assert!(c.contains(&("size", Severity::Error)));
        assert!(c.contains(&("permissions", Severity::Error)));
        assert!(c.contains(&("steps", Severity::Warning)));
        b.files.clear();
        assert!(codes(&b).contains(&("entry", Severity::Error)));
    }

    #[test]
    fn non_ascii_text_near_urls_does_not_panic() {
        let html = "<h1>বাইনারি সংখ্যা পদ্ধতি — প্রতিটি ঘর দ্বিগুণ</h1><script>// মন্তব্য\nfetch('https://x.example/a')</script><p>উৎস: //wiki</p>";
        let b = ObjectBundle::from_single_html(html, "বাইনারি", 800.0, 450.0, 0);
        let d = b.validate();
        assert!(d.iter().any(|d| d.code == "network"), "{d:?}");
    }

    #[test]
    fn prose_urls_are_not_resource_loads() {
        let b = ObjectBundle::from_single_html(
            r#"<p>Data from https://www.who.int/data</p><p>Important: see https://example.org/page</p>
               <a href="https://en.wikipedia.org/wiki/ENIAC">ENIAC</a>"#,
            "Sources",
            800.0,
            450.0,
            0,
        );
        assert!(b.is_valid(), "{:?}", b.validate());
        let b2 = ObjectBundle::from_single_html(
            r#"<script type="module">import x from "https://esm.sh/x";</script>"#,
            "x",
            300.0,
            300.0,
            0,
        );
        assert!(!b2.is_valid());
        let b3 = ObjectBundle::from_single_html(
            r#"<style>body{background:url(https://img.example/x.png)}</style>"#,
            "x",
            300.0,
            300.0,
            0,
        );
        assert!(!b3.is_valid());
        let b4 = ObjectBundle::from_single_html(
            r#"<link rel="stylesheet" href="https://fonts.googleapis.com/css">"#,
            "x",
            300.0,
            300.0,
            0,
        );
        assert!(!b4.is_valid());
    }

    #[test]
    fn unknown_lib_and_storage_warn() {
        let b = ObjectBundle::from_single_html(
            r#"<script src="rupoklib://leftpad/x.js"></script><script>localStorage.x=1</script>"#,
            "x",
            300.0,
            300.0,
            0,
        );
        let c = codes(&b);
        assert!(c.contains(&("lib", Severity::Warning)));
        assert!(c.contains(&("storage", Severity::Warning)));
        assert!(b.is_valid());
    }
}
