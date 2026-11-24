//! Reading and writing `.rpk` packages. Works on bytes so it runs unchanged in
//! WASM (web viewer) and natively.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use rupok_core::Lesson;
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::object::ObjectBundle;
use crate::readable::{self, Outline, ReadableCache};
use crate::{FORMAT_VERSION, FormatError, MIMETYPE, Result, safe_path};

const MAX_ENTRIES: usize = 50_000;
const MAX_TOTAL: u64 = 4 << 30; // 4 GiB uncompressed
const MAX_ENTRY: u64 = 1 << 30; // 1 GiB per entry

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct CourseRef {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subject: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub course: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub chapter: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub title: String,
    pub created: f64,
    pub modified: f64,
    pub generator: String,
    pub pages: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub course: Option<CourseRef>,
}

#[derive(Clone, Debug, Default)]
pub struct Package {
    pub manifest: Option<Manifest>,
    pub doc: Vec<u8>,
    pub assets: BTreeMap<String, Vec<u8>>,
    pub objects: BTreeMap<String, ObjectBundle>,
    pub thumbs: BTreeMap<String, Vec<u8>>,
    pub layers: BTreeMap<String, Vec<u8>>,
    /// Readable outline for `README.md` / `content.md` / `content.json`,
    /// refreshed by [`Package::update_lesson`]. When `None` (a package read
    /// from disk), [`Package::write`] reads it from `doc` instead.
    pub outline: Option<Outline>,
    /// Recognised handwriting kept between saves (see [`ReadableCache`]).
    pub readable_cache: ReadableCache,
    /// `now_ms` of the last outline refresh (see
    /// [`Package::update_lesson_throttled`]).
    pub outline_at: f64,
}

/// How long an autosave may reuse the readable outline before rebuilding it.
/// Rebuilding reads every item (about 0.8 s on a 100k-stroke lesson).
pub const OUTLINE_MAX_AGE_MS: f64 = 60_000.0;

impl Package {
    /// Snapshot a lesson into a package (assets/objects added separately).
    pub fn from_lesson(lesson: &Lesson, generator: &str, now_ms: f64) -> Self {
        let mut p = Package::default();
        p.update_lesson(lesson, generator, now_ms);
        p
    }

    /// Refresh the document and manifest, keeping assets/objects.
    pub fn update_lesson(&mut self, lesson: &Lesson, generator: &str, now_ms: f64) {
        self.outline = Some(Outline::from_lesson(lesson, Some(&mut self.readable_cache)));
        self.outline_at = now_ms;
        self.update_doc(lesson, generator, now_ms);
    }

    /// [`Package::update_lesson`] for frequent saves (the board's autosave):
    /// the readable outline is rebuilt only when there is none or it is
    /// older than `max_age_ms`; otherwise the last one is kept, so the
    /// readable entries may lag `doc.loro` by up to `max_age_ms` (they are
    /// derived and never authoritative). Returns whether it was rebuilt.
    pub fn update_lesson_throttled(
        &mut self,
        lesson: &Lesson,
        generator: &str,
        now_ms: f64,
        max_age_ms: f64,
    ) -> bool {
        // `abs`: a clock that jumped back still refreshes.
        let stale = self.outline.is_none() || (now_ms - self.outline_at).abs() >= max_age_ms;
        if stale {
            self.update_lesson(lesson, generator, now_ms);
        } else {
            self.update_doc(lesson, generator, now_ms);
        }
        stale
    }

    /// Refresh `doc` and the manifest only.
    fn update_doc(&mut self, lesson: &Lesson, generator: &str, now_ms: f64) {
        let course = self.manifest.as_ref().and_then(|m| m.course.clone());
        self.doc = lesson.save();
        self.manifest = Some(Manifest {
            format: "rupok-lesson".into(),
            version: FORMAT_VERSION,
            title: lesson.title(),
            created: lesson.created(),
            modified: now_ms,
            generator: generator.into(),
            pages: lesson.page_count(),
            course,
        });
    }

    /// Re-read the outline from `doc`, recognising handwriting (used after
    /// [`Package::read`], whose packages carry no outline).
    pub fn refresh_outline(&mut self) -> Result<()> {
        let lesson = self.lesson()?;
        self.outline = Some(Outline::from_lesson(
            &lesson,
            Some(&mut self.readable_cache),
        ));
        Ok(())
    }

    /// The readable outline (`content.json`), from the cached outline or,
    /// failing that, from `doc` (without handwriting recognition). `None`
    /// without a manifest or a readable document.
    pub fn content(&self) -> Option<readable::Content> {
        let manifest = self.manifest.as_ref()?;
        let loaded;
        let outline = match &self.outline {
            Some(o) => o,
            None => {
                loaded = Outline::from_lesson(&Lesson::load(&self.doc).ok()?, None);
                &loaded
            }
        };
        Some(outline.content(manifest, &self.objects))
    }

    /// `content.md`: the lesson as Markdown, for people and language models.
    pub fn content_markdown(&self) -> Option<String> {
        self.content().map(|c| readable::content_md(&c))
    }

    pub fn lesson(&self) -> Result<Lesson> {
        Ok(Lesson::load(&self.doc)?)
    }

    /// Store bytes content-addressed; returns the asset name to reference.
    pub fn add_asset(&mut self, bytes: Vec<u8>, ext: &str) -> String {
        let ext: String = ext
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(8)
            .collect::<String>()
            .to_lowercase();
        let name = format!(
            "{}.{}",
            blake3::hash(&bytes).to_hex(),
            if ext.is_empty() { "bin" } else { &ext }
        );
        self.assets.entry(name.clone()).or_insert(bytes);
        name
    }

    pub fn write(&self) -> Result<Vec<u8>> {
        let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        zw.start_file("mimetype", stored)?;
        zw.write_all(MIMETYPE.as_bytes())?;

        let manifest = self
            .manifest
            .as_ref()
            .ok_or_else(|| FormatError::NotAPackage("no manifest".into()))?;
        zw.start_file("manifest.json", deflated)?;
        zw.write_all(&serde_json::to_vec_pretty(manifest)?)?;

        // Readable entries (spec: Readable entries): derived, regenerated on
        // every write, ignored by readers.
        if let Some(content) = self.content() {
            let md = readable::content_md(&content);
            zw.start_file(readable::README_ENTRY, deflated)?;
            zw.write_all(readable::readme_md(&content, &md).as_bytes())?;
            zw.start_file(readable::CONTENT_MD_ENTRY, deflated)?;
            zw.write_all(md.as_bytes())?;
            zw.start_file(readable::CONTENT_JSON_ENTRY, deflated)?;
            zw.write_all(&readable::content_json(&content))?;
        }

        // Loro snapshots are already compact; deflate still saves ~30–50%.
        zw.start_file("doc.loro", deflated)?;
        zw.write_all(&self.doc)?;

        for (name, bytes) in &self.assets {
            let opts = if is_precompressed(name) {
                stored
            } else {
                deflated
            };
            zw.start_file(
                format!("assets/{name}"),
                opts.large_file(bytes.len() as u64 > u32::MAX as u64),
            )?;
            zw.write_all(bytes)?;
        }
        for (id, bundle) in &self.objects {
            zw.start_file(format!("objects/{id}/manifest.json"), deflated)?;
            zw.write_all(&serde_json::to_vec_pretty(&bundle.manifest)?)?;
            for (path, bytes) in &bundle.files {
                if path == "manifest.json" {
                    continue;
                }
                zw.start_file(
                    format!("objects/{id}/{path}"),
                    if is_precompressed(path) {
                        stored
                    } else {
                        deflated
                    },
                )?;
                zw.write_all(bytes)?;
            }
        }
        for (name, bytes) in &self.thumbs {
            zw.start_file(format!("thumbs/{name}"), stored)?;
            zw.write_all(bytes)?;
        }
        for (name, bytes) in &self.layers {
            zw.start_file(format!("layers/{name}"), deflated)?;
            zw.write_all(bytes)?;
        }
        Ok(zw.finish()?.into_inner())
    }

    pub fn read(bytes: &[u8]) -> Result<Self> {
        let mut za = ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| FormatError::NotAPackage(e.to_string()))?;
        if za.len() > MAX_ENTRIES {
            return Err(FormatError::Limit(format!("{} entries", za.len())));
        }
        let mut p = Package::default();
        let mut object_files: BTreeMap<String, BTreeMap<String, Vec<u8>>> = BTreeMap::new();
        let mut total = 0u64;
        let mut saw_mimetype = false;

        for i in 0..za.len() {
            let mut f = za.by_index(i)?;
            if f.is_dir() {
                continue;
            }
            let name = f.name()?.to_string();
            if !safe_path(&name) {
                return Err(FormatError::UnsafePath(name));
            }
            let declared = f.size();
            if declared > MAX_ENTRY {
                return Err(FormatError::Limit(format!("{name} is {declared} bytes")));
            }
            // Never trust the declared size alone (zip bombs): cap the reader.
            let mut buf = Vec::with_capacity(declared.min(64 << 20) as usize);
            (&mut f).take(MAX_ENTRY + 1).read_to_end(&mut buf)?;
            total += buf.len() as u64;
            if buf.len() as u64 > MAX_ENTRY || total > MAX_TOTAL {
                return Err(FormatError::Limit("uncompressed size".into()));
            }
            match name.as_str() {
                "mimetype" => {
                    if buf != MIMETYPE.as_bytes() {
                        return Err(FormatError::NotAPackage("wrong mimetype".into()));
                    }
                    saw_mimetype = true;
                }
                "manifest.json" => p.manifest = Some(serde_json::from_slice(&buf)?),
                "doc.loro" => p.doc = buf,
                // Derived entries: regenerated on write, never read back.
                readable::README_ENTRY
                | readable::CONTENT_MD_ENTRY
                | readable::CONTENT_JSON_ENTRY => {}
                _ => {
                    if let Some(rest) = name.strip_prefix("assets/") {
                        p.assets.insert(rest.to_string(), buf);
                    } else if let Some(rest) = name.strip_prefix("objects/") {
                        if let Some((id, path)) = rest.split_once('/') {
                            object_files
                                .entry(id.to_string())
                                .or_default()
                                .insert(path.to_string(), buf);
                        }
                    } else if let Some(rest) = name.strip_prefix("thumbs/") {
                        p.thumbs.insert(rest.to_string(), buf);
                    } else if let Some(rest) = name.strip_prefix("layers/") {
                        p.layers.insert(rest.to_string(), buf);
                    }
                    // Unknown entries are ignored for forward compatibility.
                }
            }
        }
        if !saw_mimetype {
            return Err(FormatError::NotAPackage("missing mimetype".into()));
        }
        let manifest = p
            .manifest
            .as_ref()
            .ok_or_else(|| FormatError::NotAPackage("missing manifest.json".into()))?;
        if manifest.version > FORMAT_VERSION {
            return Err(FormatError::TooNew(manifest.version));
        }
        if p.doc.is_empty() {
            return Err(FormatError::NotAPackage("missing doc.loro".into()));
        }
        for (id, files) in object_files {
            p.objects.insert(id, ObjectBundle::from_files(files)?);
        }
        Ok(p)
    }
}

fn is_precompressed(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "webp"
            | "avif"
            | "gif"
            | "mp3"
            | "mp4"
            | "m4a"
            | "opus"
            | "ogg"
            | "webm"
            | "woff"
            | "woff2"
            | "zip"
            | "pdf"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rupok_core::{BoxGeom, ItemData, ObjectData};

    #[test]
    fn package_round_trip() {
        let mut lesson = Lesson::new("Binary numbers", 1_700_000_000_000.0);
        let page = lesson.page_id_at(0).unwrap();
        let bundle = ObjectBundle::from_single_html(
            "<!doctype html><h1>Hi</h1><script>rupok.steps.onChange(()=>{})</script>",
            "Counter",
            800.0,
            450.0,
            3,
        );
        lesson
            .add_item(
                &page,
                &ItemData::Object(ObjectData {
                    geom: BoxGeom {
                        x: 0.0,
                        y: 0.0,
                        w: 800.0,
                        h: 450.0,
                        rot: 0.0,
                    },
                    bundle: "counter".into(),
                    title: "Counter".into(),
                    step: 0,
                    state: None,
                }),
                1.0,
            )
            .unwrap();
        let mut pkg = Package::from_lesson(&lesson, "test", 2.0);
        let a1 = pkg.add_asset(vec![1, 2, 3], "PNG");
        let a2 = pkg.add_asset(vec![1, 2, 3], "png");
        assert_eq!(a1, a2, "assets are content-addressed");
        assert!(a1.ends_with(".png"));
        pkg.objects.insert("counter".into(), bundle.clone());

        let bytes = pkg.write().unwrap();
        let back = Package::read(&bytes).unwrap();
        assert_eq!(back.manifest.as_ref().unwrap().title, "Binary numbers");
        assert_eq!(back.assets.len(), 1);
        assert_eq!(back.objects["counter"].manifest, bundle.manifest);
        assert_eq!(
            back.objects["counter"].entry_html().unwrap(),
            bundle.entry_html().unwrap()
        );
        let l2 = back.lesson().unwrap();
        assert_eq!(l2.items(&page).unwrap().len(), 1);
    }

    #[test]
    fn rejects_non_packages_and_unsafe_paths() {
        assert!(Package::read(b"hello").is_err());
        // A zip with a traversal entry.
        let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
        zw.start_file("mimetype", SimpleFileOptions::default())
            .unwrap();
        zw.write_all(MIMETYPE.as_bytes()).unwrap();
        zw.start_file("../evil", SimpleFileOptions::default())
            .unwrap();
        zw.write_all(b"x").unwrap();
        let bytes = zw.finish().unwrap().into_inner();
        assert!(matches!(
            Package::read(&bytes),
            Err(FormatError::UnsafePath(_))
        ));
    }

    /// A small lesson with every kind of item, used by the readable tests.
    fn sample_package() -> Package {
        use rupok_core::{BoxGeom, ImageData, PageSpec, ShapeGeom, StrokeData, TextData};
        use rupok_ink::math::synth::{Synth, SynthStyle};
        let mut lesson = Lesson::new("Pythagoras", 1_763_870_400_000.0);
        let p1 = lesson.page_id_at(0).unwrap();
        lesson.set_page_title(&p1, "Right triangles").unwrap();
        let g = BoxGeom {
            x: 100.0,
            y: 80.0,
            w: 800.0,
            h: 60.0,
            rot: 0.0,
        };
        lesson
            .add_item(
                &p1,
                &ItemData::Text(TextData {
                    geom: g,
                    text: "The longest side is the hypotenuse.".into(),
                    size: 40.0,
                    color: 0xff,
                    latex: false,
                }),
                1.0,
            )
            .unwrap();
        let g2 = BoxGeom { y: 200.0, ..g };
        lesson
            .add_item(
                &p1,
                &ItemData::Text(TextData {
                    geom: g2,
                    text: "a^2+b^2=c^2".into(),
                    size: 40.0,
                    color: 0xff,
                    latex: true,
                }),
                2.0,
            )
            .unwrap();
        // Handwritten "x^2", offset into the page.
        let sample = Synth::new().render("x^2", 7, &SynthStyle::clean()).unwrap();
        for (i, pts) in sample.strokes.iter().enumerate() {
            let pts = pts
                .iter()
                .map(|p| rupok_ink::StrokePoint {
                    x: p.x + 300.0,
                    y: p.y + 500.0,
                    ..*p
                })
                .collect();
            lesson
                .add_item(
                    &p1,
                    &ItemData::Stroke(StrokeData {
                        tool: "pen".into(),
                        color: 0xff,
                        width: 3.0,
                        opacity: 1.0,
                        points: pts,
                        offset: [0.0, 0.0],
                        shape: None,
                        geom: None,
                    }),
                    10.0 + i as f64,
                )
                .unwrap();
        }
        let tri = rupok_ink::StrokePoint {
            x: 1200.0,
            y: 600.0,
            p: 0.5,
            dt: 0,
        };
        for shape in ["triangle", "axes"] {
            lesson
                .add_item(
                    &p1,
                    &ItemData::Stroke(StrokeData {
                        tool: "pen".into(),
                        color: 0xff,
                        width: 3.0,
                        opacity: 1.0,
                        points: vec![tri, rupok_ink::StrokePoint { x: 1300.0, ..tri }],
                        offset: [0.0, 0.0],
                        shape: Some(shape.into()),
                        geom: Some(ShapeGeom {
                            kind: "rect".into(),
                            x: 1200.0,
                            y: 600.0,
                            w: 100.0,
                            h: 100.0,
                            ..Default::default()
                        }),
                    }),
                    20.0,
                )
                .unwrap();
        }
        let p2 = lesson
            .add_page(
                None,
                PageSpec {
                    title: "Explore".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let mut pkg = Package::from_lesson(&lesson, "test", 2.0);
        let asset = pkg.add_asset(vec![9; 16], "png");
        lesson
            .add_item(
                &p2,
                &ItemData::Image(ImageData {
                    geom: g,
                    asset,
                    media: None,
                }),
                3.0,
            )
            .unwrap();
        lesson
            .add_item(
                &p2,
                &ItemData::Object(ObjectData {
                    geom: g2,
                    bundle: "pyth".into(),
                    title: "Pythagoras proof".into(),
                    step: 0,
                    state: Some("{\"secret\":1}".into()),
                }),
                4.0,
            )
            .unwrap();
        let mut bundle = ObjectBundle::from_single_html(
            "<!doctype html><p>proof</p>",
            "Pythagoras proof",
            800.0,
            450.0,
            4,
        );
        bundle.manifest.description = "Rearranging four triangles proves a² + b² = c².".into();
        pkg.objects.insert("pyth".into(), bundle);
        lesson.add_page(None, PageSpec::default()).unwrap();
        pkg.update_lesson(&lesson, "test", 1_763_874_000_000.0);
        pkg
    }

    fn entry(bytes: &[u8], name: &str) -> Option<String> {
        let mut za = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut f = za.by_name(name).ok()?;
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        Some(s)
    }

    #[test]
    fn writes_readable_entries() {
        let bytes = sample_package().write().unwrap();
        let md = entry(&bytes, "content.md").expect("content.md");
        assert!(md.starts_with("# Pythagoras\n"), "{md}");
        for want in [
            "- Pages: 3",
            "- Created: 2025-11-23T04:00Z",
            "## Page 1: Right triangles",
            "> The longest side is the hypotenuse.",
            "$$a^2+b^2=c^2$$",
            "Handwriting: 3 ink strokes",
            "recognised maths (machine reading, may be wrong): $x^2$",
            "triangle, coordinate axes",
            "## Page 2: Explore",
            "- Image: `assets/",
            "**Pythagoras proof** (4 steps, `objects/pyth/index.html`): Rearranging four triangles",
            "## Page 3\n\n_(empty page)_",
        ] {
            assert!(md.contains(want), "missing {want:?} in\n{md}");
        }
        assert!(!md.contains("secret"), "object state is private");
        let readme = entry(&bytes, "README.md").expect("README.md");
        assert!(readme.starts_with("This is a Rupok lesson (Rich Presentation Kit"));
        assert!(readme.contains("https://rupok.app/rpk/"));
        assert!(readme.contains("https://rupok.app/skills/read-rpk.md"));
        assert!(readme.contains("## Page 1: Right triangles"));
        let json: serde_json::Value =
            serde_json::from_str(&entry(&bytes, "content.json").unwrap()).unwrap();
        assert_eq!(json["format"], "rupok-lesson-content");
        assert_eq!(json["page_count"], 3);
        assert_eq!(json["pages"][0]["ink"]["recognized"][0]["latex"], "x^2");
        assert_eq!(json["pages"][1]["objects"][0]["steps"], 4);
        let back: crate::Content = serde_json::from_value(json).unwrap();
        assert_eq!(back.pages.len(), 3);
        // The mimetype is still first and the package still reads.
        assert_eq!(&bytes[30..38], b"mimetype");
        Package::read(&bytes).unwrap().lesson().unwrap();
    }

    #[test]
    fn throttled_update_reuses_a_fresh_outline() {
        use rupok_core::{BoxGeom, ItemData, TextData};
        let mut lesson = sample_package().lesson().unwrap();
        let mut pkg = Package::default();
        let t0 = 1_763_870_400_000.0;
        // No outline yet: built.
        assert!(pkg.update_lesson_throttled(&lesson, "test", t0, OUTLINE_MAX_AGE_MS));
        let page = lesson.page_id_at(0).unwrap();
        let text = TextData {
            geom: BoxGeom {
                x: 100.0,
                y: 900.0,
                w: 400.0,
                h: 60.0,
                rot: 0.0,
            },
            text: "Added later".into(),
            size: 40.0,
            color: 0xff,
            latex: false,
        };
        lesson.add_item(&page, &ItemData::Text(text), 3.0).unwrap();
        // Within the window: doc and manifest move on, the outline is kept.
        assert!(!pkg.update_lesson_throttled(&lesson, "test", t0 + 1_000.0, OUTLINE_MAX_AGE_MS));
        assert_eq!(pkg.manifest.as_ref().unwrap().modified, t0 + 1_000.0);
        assert_eq!(
            pkg.lesson().unwrap().items(&page).unwrap().len(),
            lesson.items(&page).unwrap().len()
        );
        let md = entry(&pkg.write().unwrap(), "content.md").unwrap();
        assert!(!md.contains("Added later"), "{md}");
        // After the window (or on a full update): rebuilt.
        assert!(pkg.update_lesson_throttled(
            &lesson,
            "test",
            t0 + OUTLINE_MAX_AGE_MS,
            OUTLINE_MAX_AGE_MS
        ));
        let md = entry(&pkg.write().unwrap(), "content.md").unwrap();
        assert!(md.contains("Added later"), "{md}");
    }

    #[test]
    fn readable_entries_survive_read_then_write() {
        // A package read from disk has no outline; write() derives it.
        let bytes = sample_package().write().unwrap();
        let again = Package::read(&bytes).unwrap().write().unwrap();
        let md = entry(&again, "content.md").unwrap();
        assert!(md.contains("## Page 2: Explore"));
        assert!(md.contains("Handwriting: 3 ink strokes"));
    }

    #[test]
    fn old_packages_without_readable_entries_open() {
        // Build a package the way writers did before README/content existed.
        let lesson = Lesson::new("Old lesson", 1.0);
        let pkg = Package::from_lesson(&lesson, "rupok 0.0.1", 2.0);
        let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zw.start_file("mimetype", opts).unwrap();
        zw.write_all(MIMETYPE.as_bytes()).unwrap();
        zw.start_file("manifest.json", opts).unwrap();
        zw.write_all(&serde_json::to_vec(pkg.manifest.as_ref().unwrap()).unwrap())
            .unwrap();
        zw.start_file("doc.loro", opts).unwrap();
        zw.write_all(&pkg.doc).unwrap();
        let old = zw.finish().unwrap().into_inner();
        let back = Package::read(&old).unwrap();
        assert_eq!(back.lesson().unwrap().title(), "Old lesson");
        // Saving it adds the readable entries.
        let saved = back.write().unwrap();
        assert!(
            entry(&saved, "content.md")
                .unwrap()
                .starts_with("# Old lesson")
        );
    }

    #[test]
    fn readable_entries_are_capped() {
        let mut lesson = Lesson::new("Big", 1.0);
        let page = lesson.page_id_at(0).unwrap();
        let g = BoxGeom {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            rot: 0.0,
        };
        let huge = "ক".repeat(50_000);
        for i in 0..200 {
            lesson
                .add_item(
                    &page,
                    &ItemData::Text(rupok_core::TextData {
                        geom: BoxGeom { y: i as f64, ..g },
                        text: huge.clone(),
                        size: 10.0,
                        color: 0xff,
                        latex: false,
                    }),
                    i as f64,
                )
                .unwrap();
        }
        for _ in 0..400 {
            lesson
                .add_page(None, rupok_core::PageSpec::default())
                .unwrap();
        }
        let bytes = Package::from_lesson(&lesson, "t", 2.0).write().unwrap();
        let md = entry(&bytes, "content.md").unwrap();
        assert!(md.len() < 140 << 10, "{}", md.len());
        assert!(entry(&bytes, "README.md").unwrap().len() < 30 << 10);
        let json = entry(&bytes, "content.json").unwrap();
        assert!(json.len() <= 128 << 10);
        let c: crate::Content = serde_json::from_str(&json).unwrap();
        assert!(c.truncated);
        assert_eq!(c.page_count, 401);
    }

    #[test]
    fn rejects_newer_versions() {
        let lesson = Lesson::new("x", 0.0);
        let mut pkg = Package::from_lesson(&lesson, "test", 0.0);
        pkg.manifest.as_mut().unwrap().version = FORMAT_VERSION + 1;
        let bytes = pkg.write().unwrap();
        assert!(matches!(Package::read(&bytes), Err(FormatError::TooNew(_))));
    }
}
