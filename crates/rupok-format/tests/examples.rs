//! The example Smart Objects in `examples/objects/` must pass validation with
//! no errors, the way an agent would place them (single HTML file, wrapped by
//! `ObjectBundle::from_single_html`).

use std::path::PathBuf;

use rupok_format::{ObjectBundle, Severity};

/// (file, title, width, height, steps). Keep in sync with the presets gallery
/// in `apps/board/src/presets.ts`, which places these with the same values.
const EXAMPLES: &[(&str, &str, f64, f64, u32)] = &[
    ("binary-counter.html", "Binary counter", 1280.0, 720.0, 10),
    (
        "computing-timeline.html",
        "A short history of computing",
        1920.0,
        1080.0,
        8,
    ),
    (
        "quiz-networking.html",
        "Quiz: networking basics",
        1280.0,
        720.0,
        4,
    ),
    (
        "physics-projectile.html",
        "Projectile motion",
        1920.0,
        1080.0,
        4,
    ),
    (
        "physics-circuit.html",
        "Ohm's law: a simple circuit",
        1920.0,
        1080.0,
        6,
    ),
    (
        "chemistry-molecules-3d.html",
        "Molecules in 3D",
        1920.0,
        1080.0,
        4,
    ),
    (
        "chemistry-reaction.html",
        "Burning methane",
        1920.0,
        1080.0,
        4,
    ),
    ("math-graph.html", "Function grapher", 1920.0, 1080.0, 5),
];

/// The rupoklib libraries each example uses (detected from `rupoklib://`).
const EXAMPLE_LIBS: &[(&str, &[&str])] = &[
    ("physics-projectile.html", &[]),
    ("physics-circuit.html", &[]),
    ("chemistry-molecules-3d.html", &["katex", "three"]),
    ("chemistry-reaction.html", &[]),
    ("math-graph.html", &[]),
];

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/objects")
}

fn load(file: &str, title: &str, w: f64, h: f64, steps: u32) -> ObjectBundle {
    let path = examples_dir().join(file);
    let html = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    ObjectBundle::from_single_html(&html, title, w, h, steps)
}

#[test]
fn examples_are_valid() {
    let mut failures = Vec::new();
    for &(file, title, w, h, steps) in EXAMPLES {
        let bundle = load(file, title, w, h, steps);
        let diags = bundle.validate();
        for d in &diags {
            println!("{file}: {:?} [{}] {}", d.severity, d.code, d.message);
        }
        if !bundle.is_valid() {
            failures.push(format!("{file}: {diags:#?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "examples with validation errors:\n{}",
        failures.join("\n")
    );
}

#[test]
fn examples_have_no_warnings_and_need_no_network() {
    for &(file, title, w, h, steps) in EXAMPLES {
        let bundle = load(file, title, w, h, steps);
        let warnings: Vec<_> = bundle
            .validate()
            .into_iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect();
        assert!(
            warnings.is_empty(),
            "{file}: unexpected warnings: {warnings:#?}"
        );
        assert!(
            bundle.manifest.permissions.network.is_empty(),
            "{file}: examples must work offline"
        );
        assert!(
            bundle.entry_html().is_some_and(|h| h.contains("rupok")),
            "{file}: must use the rupok SDK"
        );
    }
}

#[test]
fn every_example_file_is_listed() {
    let mut listed: Vec<&str> = EXAMPLES.iter().map(|e| e.0).collect();
    listed.sort_unstable();
    let mut found: Vec<String> = std::fs::read_dir(examples_dir())
        .expect("examples/objects")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".html"))
        .collect();
    found.sort_unstable();
    assert_eq!(found, listed, "add new example objects to EXAMPLES");
}

#[test]
fn examples_declare_size_steps_and_libs() {
    for &(file, title, w, h, steps) in EXAMPLES {
        let bundle = load(file, title, w, h, steps);
        let m = &bundle.manifest;
        assert_eq!((m.size.w, m.size.h, m.steps), (w, h, steps), "{file}");
        let html = bundle.entry_html().unwrap_or_default();
        if steps > 0 {
            assert!(
                html.contains("rupok.steps.setCount("),
                "{file}: stepped objects declare their step count"
            );
        }
        let want: &[&str] = EXAMPLE_LIBS
            .iter()
            .find(|(f, _)| *f == file)
            .map_or(&[], |(_, libs)| *libs);
        let mut libs: Vec<&str> = m.libs.iter().map(String::as_str).collect();
        libs.sort_unstable();
        assert_eq!(libs, want, "{file}: rupoklib libraries");
    }
}
