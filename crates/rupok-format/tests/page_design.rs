//! Page design in the package (spec: "Page design", proposed): background
//! images and branding survive a write and a read, their images are stored
//! once by content, the readable outline names the branding, and lessons
//! written before the feature still open unchanged.

use std::io::{Cursor, Read};
use std::path::PathBuf;

use rupok_core::{
    BgFit, BrandAlign, BrandPos, Branding, Lesson, PageBackground, PageId, PageStyle, Pattern,
};
use rupok_format::Package;

const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0,
    1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89,
];

fn branding(logo: &str) -> Branding {
    Branding {
        logo: Some(logo.into()),
        school: "Dhaka Model School".into(),
        teacher: "Ms. Rahman".into(),
        class: "Class 8".into(),
        date: true,
        text_pos: BrandPos::Bottom,
        text_align: BrandAlign::Center,
        ..Branding::default()
    }
}

fn entry(bytes: &[u8], name: &str) -> String {
    let mut za = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    za.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

#[test]
fn design_round_trips_and_images_are_stored_once() {
    let mut lesson = Lesson::new("Design", 1.0);
    for _ in 0..3 {
        lesson.add_page(None, Default::default()).unwrap();
    }
    let mut pkg = Package::from_lesson(&lesson, "test", 1.0);
    // The same bytes added three times (a logo and a background on every page) are one asset.
    let logo = pkg.add_asset(PNG.to_vec(), "png");
    let again = pkg.add_asset(PNG.to_vec(), "PNG");
    assert_eq!(logo, again);
    let texture = pkg.add_asset(
        b"<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'/>".to_vec(),
        "svg",
    );
    let ids: Vec<PageId> = lesson.pages().into_iter().map(|p| p.id).collect();
    let style = PageStyle {
        pattern: Pattern::Grid,
        bg_color: 0x1f3b30ff,
        background: Some(PageBackground {
            image: texture.clone(),
            fit: BgFit::Tile,
            opacity: 0.5,
            dim: true,
        }),
    };
    lesson.set_page_style(&ids, &style).unwrap();
    lesson.set_branding(Some(&branding(&logo))).unwrap();
    lesson
        .set_page_branding(&ids[1], Some(&Branding::hidden()))
        .unwrap();
    pkg.update_lesson(&lesson, "test", 2.0);
    let bytes = pkg.write().unwrap();

    let back = Package::read(&bytes).unwrap();
    assert_eq!(
        back.assets.len(),
        2,
        "one logo and one texture for four pages"
    );
    assert!(back.assets.contains_key(&logo) && back.assets.contains_key(&texture));
    let l = back.lesson().unwrap();
    assert_eq!(l.branding(), Some(branding(&logo)));
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(l.page_style(id).unwrap(), style);
        let shown = l.branding_for(&l.page(id).unwrap());
        assert_eq!(shown.is_some(), i != 1, "page {i}");
    }

    // The readable outline names the branding and the background.
    let md = entry(&bytes, "content.md");
    assert!(
        md.contains(
            "Branding on every page: Dhaka Model School · Ms. Rahman · Class 8 · today's date"
        ),
        "{md}"
    );
    assert!(md.contains(&format!("logo `assets/{logo}`")));
    assert!(md.contains(&format!("Background image: `assets/{texture}`")));
    assert!(md.contains("No branding on this page"));
    let json: serde_json::Value = serde_json::from_str(&entry(&bytes, "content.json")).unwrap();
    assert_eq!(json["branding"]["school"], "Dhaka Model School");
    assert_eq!(
        json["pages"][0]["background_image"],
        format!("assets/{texture}")
    );
    assert_eq!(json["pages"][1]["branding_hidden"], true);
}

#[test]
fn a_lesson_from_before_page_design_opens_unchanged_and_takes_one() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../skills/read-rpk/tests/sample-lesson.rpk");
    let old = std::fs::read(path).unwrap();
    let pkg = Package::read(&old).unwrap();
    let mut lesson = pkg.lesson().unwrap();
    let before = lesson.pages();
    assert!(
        before
            .iter()
            .all(|p| p.background.is_none() && p.branding.is_none())
    );
    assert_eq!(lesson.branding(), None);
    let items: Vec<usize> = before
        .iter()
        .map(|p| lesson.items(&p.id).unwrap().len())
        .collect();

    let mut pkg = pkg;
    let logo = pkg.add_asset(PNG.to_vec(), "png");
    lesson.set_branding(Some(&branding(&logo))).unwrap();
    pkg.update_lesson(&lesson, "test", 3.0);
    let back = Package::read(&pkg.write().unwrap()).unwrap();
    let l = back.lesson().unwrap();
    assert_eq!(l.branding(), Some(branding(&logo)));
    let after = l.pages();
    assert_eq!(after.len(), before.len());
    for (a, b) in after.iter().zip(&before) {
        assert_eq!(
            (a.pattern, a.bg_color, &a.title),
            (b.pattern, b.bg_color, &b.title)
        );
    }
    let items_after: Vec<usize> = after
        .iter()
        .map(|p| l.items(&p.id).unwrap().len())
        .collect();
    assert_eq!(items_after, items);
}
