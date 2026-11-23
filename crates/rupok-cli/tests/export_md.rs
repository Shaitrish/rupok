//! `rupok export --md` / `rupok info --md` print a lesson as Markdown.

use std::process::Command;

fn sample() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../skills/read-rpk/tests/sample-lesson.rpk"
    )
    .into()
}

fn run(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rupok"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn export_md_prints_the_outline() {
    let md = run(&["export", "--md", &sample()]);
    assert!(md.starts_with("# Binary numbers\n"), "{md}");
    assert!(md.contains("## Page 2: Logic gates"));
    assert!(md.contains("recognised maths (machine reading, may be wrong): $x^2$"));
    assert_eq!(run(&["info", "--md", &sample()]), md);
}

#[test]
fn export_content_json() {
    let json: serde_json::Value =
        serde_json::from_str(&run(&["export", "--content-json", &sample()])).unwrap();
    assert_eq!(json["format"], "rupok-lesson-content");
    assert_eq!(
        json["pages"][2]["objects"][0]["html"],
        "objects/binary-counter/index.html"
    );
}
