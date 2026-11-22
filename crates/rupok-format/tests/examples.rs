//! The example Smart Objects in `examples/objects/` must pass validation with
//! no errors, the way an agent would place them (single HTML file, wrapped by
//! `ObjectBundle::from_single_html`).

use std::path::PathBuf;

use rupok_format::{ObjectBundle, Severity};

/// (file, title, width, height, steps)
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
