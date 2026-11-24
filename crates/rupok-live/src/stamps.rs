//! Classroom stamps for agents: the built-in set, and **custom stamps** an
//! agent (or the teacher) designs.
//!
//! A custom stamp is a definition ([`StampDef`]): a name, keywords, artwork
//! (sanitised SVG, an emoji or a short label) and a declarative behaviour
//! (placement animation, a burst, a counter badge, a sound, an auto-fade).
//! A **Smart stamp** has `art: "smart"`: its artwork is a tiny Smart Object
//! (HTML/JS) that runs in the ordinary Smart Object sandbox (no network, the
//! SDK only), placed and sized like a stamp. Agent code never runs anywhere
//! else.
//!
//! Lessons store the definitions they use in the root `stamps` map (spec:
//! Custom stamps): id → the definition as JSON, without the Smart stamp's
//! HTML, which lives in the lesson as an ordinary object bundle (shared by
//! every placement, deduplicated by content). A placed SVG, emoji or label
//! stamp is a text item whose `stamp` key is the definition's id and whose
//! `text` is the fallback (the emoji, the label or the name); a placed
//! Smart stamp is an object item.

use rupok_core::{BoxGeom, Edits, Frame, Item, ItemData, NewItem, ObjectData, TextData};
use serde_json::{Value, json};

use crate::apply::{
    BoardCtx, LiveError, Outcome, color_hex, default_ink, finite, geometry_of, has_errors,
    html_bundle, page_ref, parse_color, tag, unique_bundle_id,
};
use crate::op::{Op, StampBehavior, StampDef};

type Result<T> = std::result::Result<T, LiveError>;

/// Largest sanitised SVG a stamp may carry.
pub const MAX_SVG_BYTES: usize = 64 * 1024;
/// Largest Smart stamp HTML.
pub const MAX_HTML_BYTES: usize = 512 * 1024;
pub const ANIMATIONS: [&str; 6] = ["none", "pop", "sparkle", "bounce", "spin", "fade-in"];
pub const BURSTS: [&str; 5] = ["none", "stars", "confetti", "hearts", "sparkles"];
pub const SOUNDS: [&str; 7] = ["none", "chime", "pop", "ding", "tada", "clap", "whoosh"];
const ARTS: [&str; 4] = ["svg", "emoji", "text", "smart"];
/// Default box of a placed stamp, slide pixels.
pub const DEFAULT_SIZE: f64 = 96.0;
const DEFAULT_SMART: f64 = 200.0;

/// A built-in stamp (apps/board/src/stamps.ts keeps the same list).
pub struct Builtin {
    pub id: &'static str,
    pub label: &'static str,
    /// What a reader without stamps shows (and the default label).
    pub text: &'static str,
    pub color: u32,
    /// A pill sized to its text.
    pub pill: bool,
}

const GOLD: u32 = 0xf5b301ff;
const GREEN: u32 = 0x30a46cff;
const RED: u32 = 0xe5484dff;
const BLUE: u32 = 0x3e63ddff;
const ORANGE: u32 = 0xf76b15ff;
const PURPLE: u32 = 0x8e4ec6ff;

const fn b(id: &'static str, label: &'static str, text: &'static str, color: u32) -> Builtin {
    Builtin {
        id,
        label,
        text,
        color,
        pill: false,
    }
}
const fn pill(id: &'static str, label: &'static str, text: &'static str, color: u32) -> Builtin {
    Builtin {
        id,
        label,
        text,
        color,
        pill: true,
    }
}

pub const BUILTINS: [Builtin; 18] = [
    b("star", "Star", "⭐", GOLD),
    b("tick", "Tick", "✅", GREEN),
    b("cross", "Cross", "❌", RED),
    b("thumbs-up", "Thumbs up", "👍", GOLD),
    b("hundred", "100", "💯", RED),
    b("heart", "Heart", "❤️", RED),
    b("trophy", "Trophy", "🏆", GOLD),
    pill("excellent", "Excellent", "Excellent!", GREEN),
    pill("good-job", "Good job", "Good job!", BLUE),
    pill("try-again", "Try again", "Try again", ORANGE),
    pill("important", "Important", "⚡ Important", GOLD),
    b("question", "Question", "?", PURPLE),
    b("badge", "Number", "1", BLUE),
    b("arrow-right", "Arrow right", "→", RED),
    b("arrow-down", "Arrow down", "↓", RED),
    b("arrow-left", "Arrow left", "←", RED),
    b("arrow-up", "Arrow up", "↑", RED),
    b("emoji", "Any emoji", "⭐", 0x1c2024ff),
];

pub fn builtin(id: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.id == id)
}

/// Stamp ids follow the format's naming rule: `a-z`, `0-9`, `-`, ≤ 32 bytes.
pub fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

/// Width of a pill for `text` at height `h` (stamps.ts → stampSize).
fn pill_width(text: &str, h: f64) -> f64 {
    let chars = text.chars().count() as f64;
    (h * 1.8).max(h * 0.42 * chars + h * 0.9).round()
}

fn invalid(m: impl Into<String>) -> LiveError {
    LiveError::Invalid(m.into())
}

fn one_of(what: &str, v: &str, allowed: &[&str]) -> Result<String> {
    if allowed.contains(&v) {
        Ok(v.to_string())
    } else {
        Err(invalid(format!(
            "{what} must be one of {}, got `{v}`",
            allowed.join(", ")
        )))
    }
}

fn hex(c: &str) -> Result<String> {
    Ok(color_hex(parse_color(c)? | 0xff))
}

/// Check a behaviour and normalise it (unknown values are refused).
pub fn check_behavior(b: &StampBehavior) -> Result<StampBehavior> {
    let mut out = StampBehavior {
        counter: b.counter,
        ..Default::default()
    };
    if let Some(a) = &b.animation {
        out.animation = Some(one_of("animation", a, &ANIMATIONS)?);
    }
    if let Some(k) = &b.burst {
        out.burst = Some(one_of("burst", k, &BURSTS)?);
    }
    if b.burst_colors.len() > 6 {
        return Err(invalid("burst_colors: at most 6 colours"));
    }
    out.burst_colors = b
        .burst_colors
        .iter()
        .map(|c| hex(c))
        .collect::<Result<_>>()?;
    if let Some(s) = &b.sound {
        out.sound = Some(one_of("sound", s, &SOUNDS)?);
    }
    if let Some(f) = b.fade_after {
        if !(f.is_finite() && (0.0..=600.0).contains(&f)) {
            return Err(invalid(
                "fade_after must be between 1 and 600 seconds (0 = never)",
            ));
        }
        out.fade_after = (f >= 1.0).then_some(f);
    }
    Ok(out)
}

/// A stamp id from its name: `c-<slug>-<4 chars>`, unique among `taken`.
fn new_id(name: &str, salt: &str, taken: &dyn Fn(&str) -> bool) -> String {
    const D: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut base = crate::apply::slug(name);
    base.truncate(24);
    let base = base.trim_end_matches('-').to_string();
    for seed in 0..10_000u64 {
        let mut v = crate::apply::fnv1a(format!("{name}\n{salt}").as_bytes(), seed);
        let suffix: String = (0..4)
            .map(|_| {
                let c = D[(v % 36) as usize] as char;
                v /= 36;
                c
            })
            .collect();
        let id = format!("c-{base}-{suffix}");
        if !taken(&id) && builtin(&id).is_none() {
            return id;
        }
    }
    format!("c-{base}")
}

/// Merge `input` over `base` (an update) and check everything: the result
/// is what is stored (without `html`, which the caller turns into a bundle).
/// Returns the definition and the Smart stamp HTML, if new HTML was given.
pub fn check(input: &StampDef, base: Option<&StampDef>) -> Result<(StampDef, Option<String>)> {
    let mut d = base.cloned().unwrap_or_default();
    let pick = |a: &Option<String>, b: &Option<String>| a.clone().or_else(|| b.clone());
    d.name = pick(&input.name, &d.name);
    if !input.keywords.is_empty() {
        d.keywords = input.keywords.clone();
    }
    if let (Some(old), Some(new)) = (&d.art, &input.art)
        && old != new
        && (old == "smart" || new == "smart")
    {
        return Err(invalid(
            "a Smart stamp cannot become a picture stamp or the other way round; create a new stamp instead",
        ));
    }
    d.art = pick(&input.art, &d.art);
    if input.svg.is_some() || input.emoji.is_some() || input.text.is_some() {
        d.svg = input.svg.clone();
        d.emoji = input.emoji.clone();
        d.text = input.text.clone();
    }
    d.color = pick(&input.color, &d.color);
    d.size = input.size.or(d.size);
    d.width = input.width.or(d.width);
    d.height = input.height.or(d.height);
    if let Some(b) = &input.behavior {
        d.behavior = Some(b.clone());
    }
    d.html = None;

    let name = d.name.as_deref().map(str::trim).unwrap_or("");
    if name.is_empty() || name.chars().count() > 40 {
        return Err(invalid(
            "name must be 1 to 40 characters, e.g. \"Super star\"",
        ));
    }
    d.name = Some(name.to_string());
    if d.keywords.len() > 12 {
        return Err(invalid("keywords: at most 12"));
    }
    let mut kw = Vec::new();
    for k in &d.keywords {
        let k = k.trim().to_lowercase();
        if k.is_empty() || k.chars().count() > 24 {
            return Err(invalid("each keyword must be 1 to 24 characters"));
        }
        if !kw.contains(&k) {
            kw.push(k);
        }
    }
    d.keywords = kw;
    let art = d
        .art
        .clone()
        .ok_or_else(|| invalid("art is required: svg, emoji, text or smart"))?;
    one_of("art", &art, &ARTS)?;
    let mut html = None;
    match art.as_str() {
        "svg" => {
            let svg = d
                .svg
                .as_deref()
                .ok_or_else(|| invalid("art \"svg\" needs `svg` markup"))?;
            let clean = crate::design::sanitize_svg(svg)
                .map_err(|e| invalid(format!("svg refused: {e}")))?;
            if clean.len() > MAX_SVG_BYTES {
                return Err(invalid(format!(
                    "svg is larger than {} KB; simplify the drawing",
                    MAX_SVG_BYTES / 1024
                )));
            }
            d.svg = Some(clean);
            d.emoji = None;
            d.text = None;
        }
        "emoji" => {
            let e = d.emoji.as_deref().map(str::trim).unwrap_or("");
            if e.is_empty() || e.chars().count() > 16 {
                return Err(invalid("art \"emoji\" needs `emoji`, e.g. \"🦉\""));
            }
            d.emoji = Some(e.to_string());
            d.svg = None;
            d.text = None;
        }
        "text" => {
            let t = d.text.as_deref().map(str::trim).unwrap_or("");
            if t.is_empty() || t.chars().count() > 24 {
                return Err(invalid(
                    "art \"text\" needs `text` of 1 to 24 characters, e.g. \"Well done!\"",
                ));
            }
            d.text = Some(t.to_string());
            d.svg = None;
            d.emoji = None;
        }
        _ => {
            d.svg = None;
            d.emoji = None;
            d.text = None;
            if let Some(h) = &input.html {
                if h.len() > MAX_HTML_BYTES {
                    return Err(invalid(format!(
                        "html is larger than {} KB",
                        MAX_HTML_BYTES / 1024
                    )));
                }
                html = Some(h.clone());
            } else if d.bundle.is_none() {
                return Err(invalid(
                    "art \"smart\" needs `html`: a tiny Smart Object (read_object_spec)",
                ));
            }
            for (k, v) in [("width", d.width), ("height", d.height)] {
                if let Some(v) = v
                    && !(v.is_finite() && (40.0..=800.0).contains(&v))
                {
                    return Err(invalid(format!("{k} must be 40 to 800 slide pixels")));
                }
            }
        }
    }
    if let Some(c) = &d.color {
        d.color = Some(hex(c)?);
    }
    if let Some(s) = d.size
        && !(s.is_finite() && (24.0..=600.0).contains(&s))
    {
        return Err(invalid("size must be 24 to 600 slide pixels"));
    }
    if let Some(b) = &d.behavior {
        d.behavior = Some(check_behavior(b)?);
    }
    Ok((d, html))
}

/// The lesson's definition of a custom stamp.
pub fn lesson_def(lesson: &rupok_core::Lesson, id: &str) -> Option<StampDef> {
    let json = lesson.stamp_def(id)?;
    let mut d: StampDef = serde_json::from_str(&json).ok()?;
    d.id = Some(id.to_string());
    d.html = None;
    Some(d)
}

fn store(d: &StampDef) -> Result<String> {
    let mut s = d.clone();
    s.html = None;
    let json = serde_json::to_string(&s).map_err(|e| invalid(e.to_string()))?;
    if json.len() > rupok_core::MAX_STAMP_DEF_BYTES {
        return Err(invalid("the stamp definition is too large"));
    }
    Ok(json)
}

/// Put Smart stamp HTML in the package as a bundle (reusing an identical
/// one). Returns the bundle id and whether it is new.
fn smart_bundle(ctx: &mut BoardCtx<'_>, d: &StampDef, html: &str) -> Result<(String, bool)> {
    let name = d.name.as_deref().unwrap_or("Stamp");
    let w = d.width.unwrap_or(DEFAULT_SMART);
    let h = d.height.unwrap_or(DEFAULT_SMART);
    for (id, b) in &ctx.pkg.objects {
        if b.files.len() == 1
            && b.files.get(&b.manifest.entry).map(Vec::as_slice) == Some(html.as_bytes())
            && b.manifest.permissions.network.is_empty()
        {
            return Ok((id.clone(), false));
        }
    }
    let bundle = html_bundle(html, name, w, h, 0, Vec::new());
    let diagnostics = bundle.validate();
    if has_errors(&diagnostics) {
        return Err(LiveError::Diagnostics(diagnostics));
    }
    let id = unique_bundle_id(name, html, &ctx.pkg.objects);
    ctx.pkg.objects.insert(id.clone(), bundle);
    Ok((id, true))
}

/// Placements of a custom stamp, per page: `(page id, items)`.
fn placements(ctx: &BoardCtx<'_>, d: &StampDef) -> Result<Vec<(String, Vec<Item>)>> {
    let id = d.id.as_deref().unwrap_or("");
    let mut out = Vec::new();
    for p in ctx.lesson.pages() {
        let on: Vec<Item> = ctx
            .lesson
            .items(&p.id)?
            .into_iter()
            .filter(|i| match &i.data {
                ItemData::Text(t) => t.stamp.as_deref() == Some(id),
                ItemData::Object(o) => d.bundle.as_deref() == Some(o.bundle.as_str()),
                _ => false,
            })
            .collect();
        if !on.is_empty() {
            out.push((p.id.clone(), on));
        }
    }
    Ok(out)
}

/// What `list_stamps` says about a definition.
fn summary(d: &StampDef, placed: usize) -> Value {
    let mut v = serde_json::to_value(d).unwrap_or(Value::Null);
    if let Some(m) = v.as_object_mut() {
        if let Some(svg) = m.get("svg").and_then(Value::as_str)
            && svg.len() > 2_000
        {
            let n = svg.len();
            m.insert("svg".into(), json!(format!("({n} bytes of SVG)")));
        }
        m.insert("placements".into(), json!(placed));
        m.insert("source".into(), json!("lesson"));
    }
    v
}

pub(crate) fn run(ctx: &mut BoardCtx<'_>, op: &Op, msg: &str, now: f64) -> Result<Outcome> {
    match op {
        Op::ListStamps => {
            let builtin: Vec<Value> = BUILTINS
                .iter()
                .map(|b| {
                    json!({
                        "id": b.id, "name": b.label, "text": b.text,
                        "color": color_hex(b.color), "source": "builtin",
                    })
                })
                .collect();
            let mut custom = Vec::new();
            for (id, _) in ctx.lesson.stamp_defs() {
                if let Some(d) = lesson_def(ctx.lesson, &id) {
                    let n = placements(ctx, &d)?.iter().map(|(_, v)| v.len()).sum();
                    custom.push(summary(&d, n));
                }
            }
            Ok(Outcome::read(json!({
                "builtin": builtin,
                "custom": custom,
                "animations": ANIMATIONS,
                "bursts": BURSTS,
                "sounds": SOUNDS,
                "note": "place one with add_stamp(stamp: id); `emoji` takes the emoji as text, `badge` a number",
            })))
        }
        Op::CreateStamp(input) => {
            let (mut d, html) = check(input, None)?;
            let taken = |id: &str| ctx.lesson.stamp_def(id).is_some();
            let id = match &input.id {
                Some(id) => {
                    if !valid_id(id) {
                        return Err(invalid("id must be a-z, 0-9 and -, at most 32 characters"));
                    }
                    if builtin(id).is_some() {
                        return Err(invalid(format!(
                            "`{id}` is a built-in stamp; pick another id"
                        )));
                    }
                    if taken(id) {
                        return Err(invalid(format!(
                            "a stamp `{id}` already exists; use update_stamp to change it"
                        )));
                    }
                    id.clone()
                }
                None => new_id(d.name.as_deref().unwrap_or("stamp"), &store(&d)?, &taken),
            };
            d.id = Some(id.clone());
            let mut new_bundle = None;
            if let Some(h) = &html {
                let (b, fresh) = smart_bundle(ctx, &d, h)?;
                d.bundle = Some(b.clone());
                if fresh {
                    new_bundle = Some(b);
                }
            }
            let e = Edits {
                stamp_defs: vec![(id.clone(), Some(store(&d)?))],
                ..Default::default()
            };
            tag(ctx.lesson, msg);
            if let Err(err) = ctx.lesson.apply_edits(ctx.page_id, &e) {
                if let Some(b) = new_bundle {
                    ctx.pkg.objects.remove(&b);
                }
                return Err(err.into());
            }
            Ok(Outcome::changed(json!({
                "id": id,
                "stamp": d,
                "next": format!("place it with add_stamp(stamp: \"{id}\", x, y)"),
            })))
        }
        Op::UpdateStamp { def, base } => {
            let id = def
                .id
                .clone()
                .ok_or_else(|| invalid("id is required: the stamp to change (list_stamps)"))?;
            if builtin(&id).is_some() {
                return Err(invalid(
                    "built-in stamps cannot be changed; create_stamp makes your own",
                ));
            }
            let in_lesson = lesson_def(ctx.lesson, &id);
            let existing = in_lesson
                .clone()
                .or_else(|| base.as_deref().cloned())
                .ok_or_else(|| {
                    LiveError::NotFound(format!(
                        "no custom stamp `{id}`; list_stamps shows the ids"
                    ))
                })?;
            let (mut d, html) = check(def, Some(&existing))?;
            d.id = Some(id.clone());
            if in_lesson.is_none() {
                // Only in the teacher's device library: the board keeps it.
                return Ok(Outcome::read(
                    json!({ "id": id, "stamp": d, "in_lesson": false }),
                ));
            }
            let old = in_lesson.unwrap_or_default();
            let mut new_bundle = None;
            if let Some(h) = &html {
                let (b, fresh) = smart_bundle(ctx, &d, h)?;
                d.bundle = Some(b.clone());
                if fresh {
                    new_bundle = Some(b);
                }
            }
            let json = store(&d)?;
            // Smart stamps whose HTML changed: re-point the placements (one
            // change per page that shows the stamp).
            let mut pages = if d.bundle != old.bundle {
                placements(ctx, &old)?
                    .into_iter()
                    .map(|(p, items)| {
                        (
                            p,
                            items
                                .into_iter()
                                .filter(|i| matches!(i.data, ItemData::Object(_)))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .filter(|(_, v)| !v.is_empty())
                    .collect()
            } else {
                Vec::new()
            };
            if pages.is_empty() {
                pages.push((ctx.page_id.to_string(), Vec::new()));
            }
            let mut moved = 0;
            for (i, (page, items)) in pages.iter().enumerate() {
                let mut e = Edits {
                    stamp_defs: if i == 0 {
                        vec![(id.clone(), Some(json.clone()))]
                    } else {
                        Vec::new()
                    },
                    ..Default::default()
                };
                for it in items {
                    if let ItemData::Object(o) = &it.data {
                        e.remove.push(it.id.clone());
                        e.add.push(NewItem {
                            data: ItemData::Object(ObjectData {
                                bundle: d.bundle.clone().unwrap_or_default(),
                                title: d.name.clone().unwrap_or_default(),
                                state: None,
                                step: 0,
                                ..o.clone()
                            }),
                            created: it.created,
                            group: it.group.clone(),
                            z: Some(it.z),
                        });
                        moved += 1;
                    }
                }
                tag(ctx.lesson, msg);
                if let Err(err) = ctx.lesson.apply_edits(page, &e) {
                    if let Some(b) = &new_bundle {
                        ctx.pkg.objects.remove(b);
                    }
                    return Err(err.into());
                }
            }
            Ok(Outcome::changed(json!({
                "id": id,
                "stamp": d,
                "in_lesson": true,
                "placements_updated": moved,
            })))
        }
        Op::DeleteStamp {
            id,
            remove_placements,
            in_library,
        } => {
            if builtin(id).is_some() {
                return Err(invalid("built-in stamps cannot be deleted"));
            }
            let Some(d) = lesson_def(ctx.lesson, id) else {
                if *in_library {
                    return Ok(Outcome::read(json!({ "id": id, "in_lesson": false })));
                }
                return Err(LiveError::NotFound(format!(
                    "no custom stamp `{id}` in this lesson; list_stamps shows the ids"
                )));
            };
            let placed = placements(ctx, &d)?;
            let count: usize = placed.iter().map(|(_, v)| v.len()).sum();
            if count > 0 && !remove_placements {
                return Ok(Outcome::read(json!({
                    "id": id,
                    "in_lesson": true,
                    "kept_in_lesson": true,
                    "placements": count,
                    "note": "the lesson still shows this stamp, so it keeps its definition; pass remove_placements: true to remove the stamps from the pages too",
                })));
            }
            let mut pages = placed;
            if pages.is_empty() {
                pages.push((ctx.page_id.to_string(), Vec::new()));
            }
            for (i, (page, items)) in pages.iter().enumerate() {
                let e = Edits {
                    stamp_defs: if i == 0 {
                        vec![(id.clone(), None)]
                    } else {
                        Vec::new()
                    },
                    remove: items.iter().map(|it| it.id.clone()).collect(),
                    ..Default::default()
                };
                tag(ctx.lesson, msg);
                ctx.lesson.apply_edits(page, &e)?;
            }
            Ok(Outcome::changed(json!({
                "id": id,
                "deleted": true,
                "placements_removed": count,
            })))
        }
        Op::AddStamp {
            page,
            stamp,
            x,
            y,
            size,
            color,
            text,
            def,
        } => add(
            ctx,
            *page,
            stamp,
            *x,
            *y,
            *size,
            color,
            text,
            def.as_deref(),
            msg,
            now,
        ),
        _ => Err(LiveError::Unsupported(op.name())),
    }
}

#[allow(clippy::too_many_arguments)]
fn add(
    ctx: &mut BoardCtx<'_>,
    page: Option<u32>,
    stamp: &str,
    x: Option<f64>,
    y: Option<f64>,
    size: Option<f64>,
    color: &Option<String>,
    text: &Option<String>,
    def: Option<&StampDef>,
    msg: &str,
    now: f64,
) -> Result<Outcome> {
    let (idx, info) = page_ref(ctx.lesson, ctx.page_id, page)?;
    let frame = info.frame.unwrap_or(Frame::SLIDE_16_9);
    let size = match size {
        Some(s) if s.is_finite() && (16.0..=800.0).contains(&s) => Some(s),
        Some(_) => return Err(invalid("size must be 16 to 800 slide pixels")),
        None => None,
    };
    let color = color.as_deref().map(parse_color).transpose()?;
    if let Some(t) = text
        && (t.trim().is_empty() || t.chars().count() > 24)
    {
        return Err(invalid("text must be 1 to 24 characters"));
    }
    let place = |w: f64, h: f64| -> Result<BoxGeom> {
        Ok(BoxGeom {
            x: finite("x", x.unwrap_or((frame.w - w) / 2.0))?,
            y: finite("y", y.unwrap_or((frame.h - h) / 2.0))?,
            w,
            h,
            rot: 0.0,
        })
    };
    let mut e = Edits::default();
    let mut new_bundle = None;
    let mut behavior = None;
    let data = if let Some(b) = builtin(stamp) {
        let h = size.unwrap_or(if b.pill { 76.0 } else { DEFAULT_SIZE });
        let t = match b.id {
            "emoji" => text
                .clone()
                .ok_or_else(|| invalid("the emoji stamp needs `text`: the emoji, e.g. \"🦉\""))?,
            "badge" => match text {
                Some(t) => t.clone(),
                None => next_badge(&ctx.lesson.items(&info.id)?).to_string(),
            },
            _ if b.pill => text.clone().unwrap_or_else(|| b.text.to_string()),
            _ => b.text.to_string(),
        };
        let w = if b.pill { pill_width(&t, h) } else { h };
        ItemData::Text(TextData {
            geom: place(w, h)?,
            text: t,
            size: (h * 0.7).round(),
            color: color.unwrap_or(b.color),
            latex: false,
            stamp: Some(b.id.to_string()),
        })
    } else {
        if !valid_id(stamp) {
            return Err(invalid(format!(
                "no stamp `{stamp}`; list_stamps shows the ids"
            )));
        }
        let (d, html) = match lesson_def(ctx.lesson, stamp) {
            Some(d) => (d, None),
            None => {
                let Some(given) = def else {
                    return Err(LiveError::NotFound(format!(
                        "no stamp `{stamp}`; list_stamps shows the ids (create_stamp makes a new one)"
                    )));
                };
                let (mut d, html) = check(given, None)?;
                d.id = Some(stamp.to_string());
                (d, html)
            }
        };
        let mut d = d;
        let fresh_def = lesson_def(ctx.lesson, stamp).is_none();
        behavior = d.behavior.clone();
        let name = d.name.clone().unwrap_or_default();
        if d.art.as_deref() == Some("smart") {
            let have = d
                .bundle
                .as_ref()
                .is_some_and(|b| ctx.pkg.objects.contains_key(b));
            if !have {
                let h = html.ok_or_else(|| {
                    invalid(format!(
                        "the Smart stamp `{stamp}` has no HTML in this lesson; update_stamp with html first"
                    ))
                })?;
                let (b, fresh) = smart_bundle(ctx, &d, &h)?;
                d.bundle = Some(b.clone());
                if fresh {
                    new_bundle = Some(b);
                }
            }
            if fresh_def || d.bundle != lesson_def(ctx.lesson, stamp).and_then(|o| o.bundle) {
                e.stamp_defs.push((stamp.to_string(), Some(store(&d)?)));
            }
            let (dw, dh) = (
                d.width.unwrap_or(DEFAULT_SMART),
                d.height.unwrap_or(DEFAULT_SMART),
            );
            let h = size.or(d.size).unwrap_or(dh);
            let w = (dw * h / dh).round();
            ItemData::Object(ObjectData {
                geom: place(w, h)?,
                bundle: d.bundle.clone().unwrap_or_default(),
                title: name,
                step: 0,
                state: None,
            })
        } else {
            if fresh_def {
                e.stamp_defs.push((stamp.to_string(), Some(store(&d)?)));
            }
            let h = size.or(d.size).unwrap_or(DEFAULT_SIZE);
            let (fallback, w) = match d.art.as_deref() {
                Some("text") => {
                    let t = d.text.clone().unwrap_or(name.clone());
                    let w = pill_width(&t, h);
                    (t, w)
                }
                Some("emoji") => (d.emoji.clone().unwrap_or(name.clone()), h),
                _ => (name.clone(), h),
            };
            let tint = d
                .color
                .as_deref()
                .and_then(|c| parse_color(c).ok())
                .unwrap_or_else(|| default_ink(info.bg_color));
            ItemData::Text(TextData {
                geom: place(w, h)?,
                text: fallback,
                size: (h * 0.7).round(),
                color: color.unwrap_or(tint),
                latex: false,
                stamp: Some(stamp.to_string()),
            })
        }
    };
    let geom = match &data {
        ItemData::Text(t) => t.geom,
        ItemData::Object(o) => o.geom,
        _ => BoxGeom::default(),
    };
    e.add.push(NewItem {
        data,
        created: now,
        group: None,
        z: None,
    });
    tag(ctx.lesson, msg);
    let ids = match ctx.lesson.apply_edits(&info.id, &e) {
        Ok(ids) => ids,
        Err(err) => {
            if let Some(b) = new_bundle {
                ctx.pkg.objects.remove(&b);
            }
            return Err(err.into());
        }
    };
    let count = ctx
        .lesson
        .items(&info.id)?
        .iter()
        .filter(|i| match &i.data {
            ItemData::Text(t) => t.stamp.as_deref() == Some(stamp),
            _ => false,
        })
        .count();
    Ok(Outcome::changed(json!({
        "item_id": ids.first(),
        "stamp": stamp,
        "page": idx + 1,
        "geometry": geometry_of(&geom),
        "count_on_page": count,
        "behavior": behavior,
    })))
}

/// The next number for a badge: one more than the highest on the page.
fn next_badge(items: &[Item]) -> u32 {
    items
        .iter()
        .filter_map(|i| match &i.data {
            ItemData::Text(t) if t.stamp.as_deref() == Some("badge") => t.text.trim().parse().ok(),
            _ => None,
        })
        .max()
        .unwrap_or(0)
        + 1
}
