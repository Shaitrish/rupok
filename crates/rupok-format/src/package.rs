//! Reading and writing `.rpk` packages. Works on bytes so it runs unchanged in
//! WASM (web viewer) and natively.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use rupok_core::Lesson;
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::object::ObjectBundle;
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
}

impl Package {
    /// Snapshot a lesson into a package (assets/objects added separately).
    pub fn from_lesson(lesson: &Lesson, generator: &str, now_ms: f64) -> Self {
        let mut p = Package::default();
        p.update_lesson(lesson, generator, now_ms);
        p
    }

    /// Refresh the document and manifest, keeping assets/objects.
    pub fn update_lesson(&mut self, lesson: &Lesson, generator: &str, now_ms: f64) {
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

    #[test]
    fn rejects_newer_versions() {
        let lesson = Lesson::new("x", 0.0);
        let mut pkg = Package::from_lesson(&lesson, "test", 0.0);
        pkg.manifest.as_mut().unwrap().version = FORMAT_VERSION + 1;
        let bytes = pkg.write().unwrap();
        assert!(matches!(Package::read(&bytes), Err(FormatError::TooNew(_))));
    }
}
