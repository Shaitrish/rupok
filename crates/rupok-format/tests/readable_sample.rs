//! The sample lesson used by the `read-rpk` agent skill
//! (`skills/read-rpk/tests/sample-lesson.rpk`) and a check that it still
//! opens and carries readable entries.
//!
//! Regenerate the fixture after a format change:
//! `RUPOK_WRITE_SAMPLE=1 cargo test -p rupok-format --test readable_sample`

use std::io::{Cursor, Read};
use std::path::PathBuf;

use rupok_core::{
    BoxGeom, ImageData, ItemData, Lesson, ObjectData, PageSpec, Pattern, ShapeGeom, StrokeData,
    TextData,
};
use rupok_format::{CourseRef, ObjectBundle, Package};
use rupok_ink::StrokePoint;
use rupok_ink::math::synth::{Synth, SynthStyle};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/read-rpk/tests/sample-lesson.rpk")
}

fn text(lesson: &mut Lesson, page: &str, y: f64, s: &str, latex: bool, t: f64) {
    let geom = BoxGeom {
        x: 120.0,
        y,
        w: 1400.0,
        h: 70.0,
        rot: 0.0,
    };
    let data = TextData {
        geom,
        text: s.into(),
        size: 44.0,
        color: 0x1d2433ff,
        latex,
    };
    lesson.add_item(page, &ItemData::Text(data), t).unwrap();
}

fn stroke(points: Vec<StrokePoint>, shape: Option<&str>) -> ItemData {
    ItemData::Stroke(StrokeData {
        tool: "pen".into(),
        color: 0x1d2433ff,
        width: 3.0,
        opacity: 1.0,
        points,
        offset: [0.0, 0.0],
        shape: shape.map(Into::into),
        geom: shape.map(|_| ShapeGeom {
            kind: "rect".into(),
            x: 1300.0,
            y: 500.0,
            w: 400.0,
            h: 300.0,
            ..Default::default()
        }),
    })
}

fn sample() -> Vec<u8> {
    let t0 = 1_763_870_400_000.0; // 2025-11-23
    let mut lesson = Lesson::new("Binary numbers", t0);
    let p1 = lesson.page_id_at(0).unwrap();
    lesson.set_page_title(&p1, "Place value in base 2").unwrap();
    text(
        &mut lesson,
        &p1,
        80.0,
        "Each place is worth twice the place to its right.",
        false,
        t0 + 1.0,
    );
    text(
        &mut lesson,
        &p1,
        180.0,
        "1011_2 = 8 + 2 + 1 = 11",
        true,
        t0 + 2.0,
    );
    // The teacher writes x^2 by hand next to it.
    let sample = Synth::new().render("x^2", 7, &SynthStyle::clean()).unwrap();
    for (i, pts) in sample.strokes.iter().enumerate() {
        let pts = pts
            .iter()
            .map(|p| StrokePoint {
                x: p.x + 200.0,
                y: p.y + 500.0,
                ..*p
            })
            .collect();
        lesson
            .add_item(&p1, &stroke(pts, None), t0 + 10.0 + i as f64)
            .unwrap();
    }
    let a = StrokePoint {
        x: 1300.0,
        y: 800.0,
        p: 0.5,
        dt: 0,
    };
    let b = StrokePoint {
        x: 1700.0,
        y: 500.0,
        p: 0.5,
        dt: 40,
    };
    lesson
        .add_item(&p1, &stroke(vec![a, b], Some("axes")), t0 + 20.0)
        .unwrap();

    let p2 = lesson
        .add_page(
            None,
            PageSpec {
                title: "Logic gates".into(),
                pattern: Pattern::Grid,
                ..Default::default()
            },
        )
        .unwrap();
    text(
        &mut lesson,
        &p2,
        80.0,
        "An AND gate is on only when both inputs are on.",
        false,
        t0 + 30.0,
    );
    for shape in ["and-gate", "or-gate", "arrow", "arrow"] {
        lesson
            .add_item(&p2, &stroke(vec![a, b], Some(shape)), t0 + 31.0)
            .unwrap();
    }

    let p3 = lesson
        .add_page(
            None,
            PageSpec {
                title: "Try it".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let mut pkg = Package::from_lesson(&lesson, "rupok 0.1.0", t0);
    // A tiny valid PNG (1×1, transparent).
    let png: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52, 0,
        0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89, 0, 0, 0, 0x0a, 0x49, 0x44,
        0x41, 0x54, 0x78, 0x9c, 0x63, 0, 1, 0, 0, 5, 0, 1, 0x0d, 0x0a, 0x2d, 0xb4, 0, 0, 0, 0,
        0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    let asset = pkg.add_asset(png.to_vec(), "png");
    let g = BoxGeom {
        x: 120.0,
        y: 120.0,
        w: 480.0,
        h: 320.0,
        rot: 0.0,
    };
    lesson
        .add_item(
            &p3,
            &ItemData::Image(ImageData { geom: g, asset }),
            t0 + 40.0,
        )
        .unwrap();
    let html = include_str!("../../../examples/objects/binary-counter.html");
    let mut bundle = ObjectBundle::from_single_html(html, "Binary counter", 1280.0, 720.0, 8);
    bundle.manifest.description =
        "Eight switches count from 0 to 255 in binary; each step lights one more place.".into();
    pkg.objects.insert("binary-counter".into(), bundle);
    let og = BoxGeom {
        x: 640.0,
        y: 120.0,
        w: 1280.0,
        h: 720.0,
        rot: 0.0,
    };
    let obj = ObjectData {
        geom: og,
        bundle: "binary-counter".into(),
        title: "Binary counter".into(),
        step: 0,
        state: None,
    };
    lesson
        .add_item(&p3, &ItemData::Object(obj), t0 + 41.0)
        .unwrap();

    pkg.manifest.as_mut().unwrap().course = Some(CourseRef {
        subject: "ICT".into(),
        course: "Grade 9 Computing".into(),
        chapter: "Number systems".into(),
    });
    pkg.update_lesson(&lesson, "rupok 0.1.0", t0 + 3_600_000.0);
    pkg.write().unwrap()
}

#[test]
fn sample_fixture_is_current_and_readable() {
    let bytes = sample();
    if std::env::var_os("RUPOK_WRITE_SAMPLE").is_some() {
        std::fs::write(fixture(), &bytes).unwrap();
    }
    let on_disk = std::fs::read(fixture()).expect("run with RUPOK_WRITE_SAMPLE=1 once");
    let pkg = Package::read(&on_disk).unwrap();
    assert_eq!(pkg.lesson().unwrap().page_count(), 3);
    let mut za = zip::ZipArchive::new(Cursor::new(&on_disk)).unwrap();
    let mut md = String::new();
    za.by_name("content.md")
        .unwrap()
        .read_to_string(&mut md)
        .unwrap();
    assert!(md.contains("# Binary numbers"));
    assert!(md.contains("Binary counter"));
    // The fixture matches what today's writer produces.
    assert_eq!(
        pkg.content_markdown().unwrap().lines().next(),
        md.lines().next()
    );
}
