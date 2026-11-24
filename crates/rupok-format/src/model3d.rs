//! 3D models (glTF 2.0) as built-in Smart Objects.
//!
//! A model is an ordinary Smart Object bundle: `index.html` is the Rupok 3D
//! viewer (three.js from `rupoklib://`, so it works offline) with the model
//! inlined as base64. Nothing new in the format: it saves, exports (via the
//! object snapshot), copies and reaches agents like any other object, and it
//! keeps the Smart Object sandbox (no network, opaque origin).
//!
//! The viewer: orbit, zoom, pan, auto-rotate, reset view, the model's own
//! animations, light or dark backdrop, and saved views as build steps (the
//! presenter's clicker steps through camera angles).

use crate::object::{ObjectBundle, ObjectManifest, Permissions, Size};
use crate::{FormatError, Result};
use std::collections::BTreeMap;

/// Largest model accepted. Base64 makes it a third bigger inside the
/// viewer, which must stay under the 25 MiB bundle limit.
pub const MAX_MODEL_BYTES: usize = 18 << 20;

const VIEWER: &str = include_str!("model-viewer.html");

/// The model format by content: `glb` (binary glTF) or `gltf` (JSON glTF
/// with every buffer and image embedded as a `data:` URI).
pub fn sniff_model(bytes: &[u8]) -> Result<&'static str> {
    if bytes.starts_with(b"glTF") {
        if bytes.len() < 20 {
            return Err(FormatError::InvalidObject(
                "the .glb file is truncated".into(),
            ));
        }
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != 2 {
            return Err(FormatError::InvalidObject(format!(
                "glTF version {version} is not supported (only glTF 2.0)"
            )));
        }
        return Ok("glb");
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| FormatError::InvalidObject("not a .glb or .gltf model".into()))?;
    let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|_| FormatError::InvalidObject("not a .glb or .gltf model".into()))?;
    if json.pointer("/asset/version").and_then(|v| v.as_str()) != Some("2.0") {
        return Err(FormatError::InvalidObject(
            "only glTF 2.0 models are supported".into(),
        ));
    }
    for list in ["buffers", "images"] {
        for entry in json
            .get(list)
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            if let Some(uri) = entry.get("uri").and_then(|v| v.as_str())
                && !uri.starts_with("data:")
            {
                return Err(FormatError::InvalidObject(format!(
                    "this .gltf refers to another file (`{}`). Export the model as one .glb file instead",
                    uri.chars().take(60).collect::<String>()
                )));
            }
        }
    }
    Ok("gltf")
}

/// A title from a file name: `"heart_model.glb"` → `"Heart model"`.
pub fn title_from_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = base.rsplit_once('.').map_or(base, |(s, _)| s);
    let words = stem.replace(['_', '-'], " ");
    let t = words.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut c = t.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => "3D model".into(),
    }
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The viewer object for a model file. `name` is the original file name
/// (shown to the teacher; only its last segment is kept). Errors when the
/// file is not a usable glTF 2.0 model or is too big.
pub fn model_bundle(model: &[u8], name: &str, title: &str, w: f64, h: f64) -> Result<ObjectBundle> {
    if model.len() > MAX_MODEL_BYTES {
        return Err(FormatError::InvalidObject(format!(
            "the model is {:.1} MB; the limit is {} MB",
            model.len() as f64 / 1_048_576.0,
            MAX_MODEL_BYTES >> 20
        )));
    }
    let format = sniff_model(model)?;
    let file: String = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("model.glb")
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect();
    let title = if title.trim().is_empty() {
        title_from_name(&file)
    } else {
        title.trim().to_string()
    };
    let html = VIEWER
        .replace("{{TITLE}}", &html_escape(&title))
        .replace("{{FILE}}", &html_escape(&file))
        .replace("{{FORMAT}}", format)
        .replace("{{MODEL}}", &base64(model));
    let manifest = ObjectManifest {
        schema: crate::object::OBJECT_SCHEMA,
        title: title.clone(),
        description: format!("3D model viewer ({file}): orbit, zoom, animations and saved views."),
        entry: "index.html".into(),
        size: Size { w, h },
        steps: 0,
        libs: vec!["three".into()],
        permissions: Permissions::default(),
        theme: Vec::new(),
        generator: Some("rupok-3d-viewer".into()),
    };
    let mut files = BTreeMap::new();
    files.insert("index.html".to_string(), html.into_bytes());
    Ok(ObjectBundle { manifest, files })
}

/// Whether a bundle is a 3D model viewer made by [`model_bundle`].
pub fn is_model_bundle(b: &ObjectBundle) -> bool {
    b.manifest.generator.as_deref() == Some("rupok-3d-viewer")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;

    /// A minimal valid glTF 2.0 binary (one triangle).
    pub fn tiny_glb() -> Vec<u8> {
        let json = br#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[0]}],"scene":0,"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}],"bufferViews":[{"buffer":0,"byteLength":36}],"buffers":[{"byteLength":36}]}"#;
        let mut j = json.to_vec();
        while !j.len().is_multiple_of(4) {
            j.push(b' ');
        }
        let mut bin = Vec::new();
        for v in [0f32, 0., 0., 1., 0., 0., 0., 1., 0.] {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        let total = 12 + 8 + j.len() + 8 + bin.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(j.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&j);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }

    #[test]
    fn builds_a_valid_viewer_object() {
        let b = model_bundle(
            &tiny_glb(),
            "C:\\models\\heart_model.glb",
            "",
            1280.0,
            720.0,
        )
        .unwrap();
        assert_eq!(b.manifest.title, "Heart model");
        assert!(is_model_bundle(&b));
        let errors: Vec<_> = b
            .validate()
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
        let html = b.entry_html().unwrap();
        assert!(html.contains("rupoklib://three/three.module.js"));
        assert!(html.contains("rupoklib://three/addons/loaders/GLTFLoader.js"));
        assert!(html.contains(&base64(&tiny_glb())));
        assert!(html.contains(r#"data-format="glb""#));
        assert!(!html.contains("{{"));
    }

    #[test]
    fn refuses_other_files() {
        assert!(model_bundle(b"<html>", "x.glb", "x", 100.0, 100.0).is_err());
        let mut v1 = tiny_glb();
        v1[4] = 1;
        assert!(sniff_model(&v1).is_err());
        assert!(sniff_model(b"glTF\x02").is_err());
        let external =
            br#"{"asset":{"version":"2.0"},"buffers":[{"uri":"scene.bin","byteLength":4}]}"#;
        assert!(
            sniff_model(external)
                .unwrap_err()
                .to_string()
                .contains("scene.bin")
        );
        let embedded = br#"{"asset":{"version":"2.0"},"buffers":[{"uri":"data:application/octet-stream;base64,AAAA","byteLength":3}]}"#;
        assert_eq!(sniff_model(embedded).unwrap(), "gltf");
        let big = vec![0u8; MAX_MODEL_BYTES + 1];
        assert!(model_bundle(&big, "x.glb", "x", 100.0, 100.0).is_err());
    }

    #[test]
    fn escapes_names_and_encodes_base64() {
        let b = model_bundle(&tiny_glb(), "<b>.glb", "A \"cell\" <model>", 640.0, 480.0).unwrap();
        let html = b.entry_html().unwrap();
        assert!(html.contains("A &quot;cell&quot; &lt;model&gt;"));
        assert!(!html.contains("<b>.glb"));
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(title_from_name(".glb"), "3D model");
    }
}
