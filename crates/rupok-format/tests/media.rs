//! Media items and 3D models in a `.rpk` package: round trips, the readable
//! outline, and what a reader from before media items sees.

use rupok_core::{BoxGeom, ImageData, ItemData, Lesson, MediaData, ObjectData};
use rupok_format::{Package, media, model3d};

fn geom() -> BoxGeom {
    BoxGeom {
        x: 320.0,
        y: 180.0,
        w: 1280.0,
        h: 720.0,
        rot: 0.0,
    }
}

/// A minimal glTF 2.0 binary: one triangle.
fn tiny_glb() -> Vec<u8> {
    let mut j = br#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[0]}],"scene":0,"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}],"bufferViews":[{"buffer":0,"byteLength":36}],"buffers":[{"byteLength":36}]}"#.to_vec();
    while !j.len().is_multiple_of(4) {
        j.push(b' ');
    }
    let bin: Vec<u8> = [0f32, 0., 0., 1., 0., 0., 0., 1., 0.]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let mut out = b"glTF".to_vec();
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&((28 + j.len() + bin.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(j.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&j);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

/// A lesson with a YouTube video, an MP4 clip, a PhET embed and a 3D model.
fn lesson_with_media() -> (Package, Vec<String>) {
    let mut lesson = Lesson::new("Plants", 1.0);
    let page = lesson.page_id_at(0).unwrap();
    let mut pkg = Package::from_lesson(&lesson, "test", 1.0);
    let poster = pkg.add_asset(b"\x89PNG poster".to_vec(), "png");
    let mut mp4 = vec![0, 0, 0, 0x18];
    mp4.extend_from_slice(b"ftypisom-clip-bytes");
    let (kind, ext) = media::sniff_media(&mp4).unwrap();
    let clip = pkg.add_asset(mp4, ext);
    let link = media::parse_link("https://youtu.be/dQw4w9WgXcQ?t=1m").unwrap();
    let yt = MediaData {
        kind: link.kind.into(),
        src: link.url.clone(),
        title: "How plants make food".into(),
        site: link.site.into(),
        start: link.start,
        end: Some(95.0),
        ..MediaData::default()
    };
    let phet =
        media::parse_link("https://phet.colorado.edu/en/simulations/projectile-motion").unwrap();
    let mut ids = Vec::new();
    for (asset, m) in [
        (poster.clone(), yt),
        (
            poster.clone(),
            MediaData {
                kind: kind.into(),
                src: clip.clone(),
                title: "Leaf time-lapse".into(),
                looped: true,
                ..MediaData::default()
            },
        ),
        (
            poster.clone(),
            MediaData {
                kind: phet.kind.into(),
                src: phet.url,
                title: phet.title,
                site: phet.site.into(),
                ..MediaData::default()
            },
        ),
    ] {
        let data = ItemData::Image(ImageData {
            geom: geom(),
            asset,
            media: Some(m),
        });
        ids.push(lesson.add_item(&page, &data, 2.0).unwrap());
    }
    let bundle = model3d::model_bundle(&tiny_glb(), "leaf_cell.glb", "", 1280.0, 720.0).unwrap();
    pkg.objects.insert("leaf-cell".into(), bundle);
    ids.push(
        lesson
            .add_item(
                &page,
                &ItemData::Object(ObjectData {
                    geom: geom(),
                    bundle: "leaf-cell".into(),
                    title: "Leaf cell".into(),
                    step: 0,
                    state: Some(r#"{"views":[{"pos":[1,2,3],"target":[0,0,0]}]}"#.into()),
                }),
                3.0,
            )
            .unwrap(),
    );
    pkg.update_lesson(&lesson, "test", 2.0);
    (pkg, ids)
}

#[test]
fn media_and_models_round_trip_through_a_package() {
    let (pkg, ids) = lesson_with_media();
    let bytes = pkg.write().unwrap();
    let back = Package::read(&bytes).unwrap();
    assert_eq!(back.assets, pkg.assets);
    assert_eq!(back.objects, pkg.objects);
    let a = pkg.lesson().unwrap();
    let b = back.lesson().unwrap();
    let page = b.page_id_at(0).unwrap();
    assert_eq!(a.items(&page).unwrap(), b.items(&page).unwrap());
    let ItemData::Image(yt) = b.item(&page, &ids[0]).unwrap().data else {
        panic!("not an image")
    };
    let m = yt.media.unwrap();
    assert_eq!(
        (m.kind.as_str(), m.start, m.end),
        ("youtube", Some(60.0), Some(95.0))
    );
    let ItemData::Image(clip) = b.item(&page, &ids[1]).unwrap().data else {
        panic!("not an image")
    };
    let clip = clip.media.unwrap();
    assert!(back.assets.contains_key(clip.src_asset().unwrap()));
    assert!(clip.src.ends_with(".mp4") && clip.looped);
    // The 3D viewer object survives with its model inside.
    let bundle = &back.objects["leaf-cell"];
    assert!(model3d::is_model_bundle(bundle));
    assert!(
        bundle
            .validate()
            .iter()
            .all(|d| d.severity != rupok_format::Severity::Error)
    );
}

#[test]
fn readable_outline_lists_links_and_files() {
    let (pkg, _) = lesson_with_media();
    let md = pkg.content_markdown().unwrap();
    assert!(md.contains("- YouTube video: **How plants make food** <https://www.youtube.com/watch?v=dQw4w9WgXcQ> (plays 1:00 to 1:35)"), "{md}");
    assert!(md.contains("- Video: **Leaf time-lapse** `assets/"), "{md}");
    assert!(md.contains("- Web embed (PhET): **Projectile Motion** <https://phet.colorado.edu/sims/html/projectile-motion/latest/projectile-motion_all.html>"), "{md}");
    assert!(md.contains("Smart Object: **Leaf cell**"), "{md}");
    // Media posters are not listed again as images.
    assert!(!md.contains("- Image:"), "{md}");
    let content = pkg.content().unwrap();
    let json = serde_json::to_value(&content).unwrap();
    let media = &json["pages"][0]["media"];
    assert_eq!(media[0]["kind"], "youtube");
    assert_eq!(
        media[0]["url"],
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    );
    assert!(media[1]["file"].as_str().unwrap().starts_with("assets/"));
    assert_eq!(media[2]["site"], "phet");
}

/// A reader from before media items knows `image` items only by `asset`:
/// the item is still an image with its poster, so it shows the poster.
#[test]
fn older_readers_see_the_poster() {
    let (pkg, ids) = lesson_with_media();
    let doc = loro::LoroDoc::new();
    doc.import(&pkg.doc).unwrap();
    let json = serde_json::to_value(doc.get_deep_value()).unwrap();
    let items = &json["pages"][0]["items"];
    for id in &ids[..3] {
        assert_eq!(items[id]["kind"], "image");
        assert!(items[id]["asset"].as_str().unwrap().ends_with(".png"));
        for k in ["x", "y", "w", "h"] {
            assert!(items[id][k].is_number());
        }
    }
    // And a page that only an older writer touched (no `media` key) still reads.
    let mut lesson = Lesson::new("old", 1.0);
    let page = lesson.page_id_at(0).unwrap();
    let id = lesson
        .add_item(
            &page,
            &ItemData::Image(ImageData {
                geom: geom(),
                asset: "a.png".into(),
                media: None,
            }),
            1.0,
        )
        .unwrap();
    let back = Lesson::load(&lesson.save()).unwrap();
    let ItemData::Image(i) = back.item(&page, &id).unwrap().data else {
        panic!()
    };
    assert_eq!(i.media, None);
}
