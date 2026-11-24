//! Page design by an agent: `set_page_design` (colour, pattern, background
//! image) and `set_branding` (logo and header/footer text). Spec:
//! `rupok-format.md`, "Page design".
//!
//! Agents draw backgrounds and logos as SVG. An SVG from an agent is
//! untrusted: [`sanitize_svg`] refuses scripts, event handlers, external
//! references and animation (an animated background would repaint the
//! board on every frame), and gives the root a size and namespace so every
//! renderer (board, thumbnails, PDF) agrees on it. PNG and JPEG bytes come
//! base64-encoded and are checked by their signature. Images become package
//! assets, stored once by content.
//!
//! Each write is one Loro commit tagged with the agent: one undo step,
//! attributed in the history.

use base64::Engine as _;
use rupok_core::{
    BgFit, BrandAlign, BrandPos, Branding, Frame, PageBackground, PageId, PageStyle, Pattern,
};
use serde_json::{Value, json};

use crate::apply::{BoardCtx, LiveError, Outcome, board_color, color_hex, is_dark, page_ref, tag};
use crate::op::{BrandingArgs, Op, PageDesignArgs};

type Result<T> = std::result::Result<T, LiveError>;

/// Largest SVG accepted (bytes).
pub const MAX_SVG_BYTES: usize = 1 << 20;
/// Largest PNG or JPEG accepted (decoded bytes).
pub const MAX_IMAGE_BYTES: usize = 8 << 20;

fn invalid(s: impl Into<String>) -> LiveError {
    LiveError::Invalid(s.into())
}

// ------------------------------------------------------------------ SVG

/// Check an agent's SVG and return it ready to store: refuse anything that
/// could run code, load from the network or animate; add `xmlns` when
/// missing and an explicit `width`/`height` (from the `viewBox`, else
/// 1920 × 1080) when the root has none.
pub fn sanitize_svg(svg: &str) -> std::result::Result<String, String> {
    let s = svg.trim().trim_start_matches('\u{feff}');
    if s.len() > MAX_SVG_BYTES {
        return Err(format!("the SVG is {} bytes; the limit is 1 MiB", s.len()));
    }
    let lower = s.to_ascii_lowercase();
    let start = lower
        .find("<svg")
        .ok_or("not an SVG: there is no <svg> element")?;
    for bad in [
        "<script",
        "<foreignobject",
        "<iframe",
        "<embed",
        "<object",
        "<audio",
        "<video",
        "<!doctype",
        "<!entity",
        "<?xml-stylesheet",
        "javascript:",
        "vbscript:",
        "@import",
        "<animate",
        "<set",
        "@keyframes",
        "animation:",
        "animation-name",
        "<handler",
        "<listener",
    ] {
        if lower.contains(bad) {
            return Err(format!(
                "the SVG contains `{bad}`; scripts, embedded documents, external styles and animation are not allowed in a page design"
            ));
        }
    }
    if let Some(attr) = event_attribute(&lower) {
        return Err(format!(
            "the SVG has an event attribute `{attr}`; remove event handlers"
        ));
    }
    if let Some(r) = external_reference(&lower) {
        return Err(format!(
            "the SVG refers to `{r}`; only references inside the SVG (`#id`) and embedded PNG/JPEG data are allowed"
        ));
    }
    normalize_root(s, start).ok_or_else(|| "the <svg> start tag could not be read".into())
}

/// ` onload=`, ` onclick =`…: an attribute starting with `on`.
fn event_attribute(lower: &str) -> Option<String> {
    let b = lower.as_bytes();
    for i in 0..b.len().saturating_sub(3) {
        if !b[i].is_ascii_whitespace() || &b[i + 1..i + 3] != b"on" {
            continue;
        }
        let mut j = i + 3;
        while j < b.len() && b[j].is_ascii_alphabetic() {
            j += 1;
        }
        let name_end = j;
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if name_end > i + 3 && j < b.len() && b[j] == b'=' {
            return Some(lower[i + 1..name_end].to_string());
        }
    }
    None
}

/// The first `href` or `url(…)` that leaves the SVG.
fn external_reference(lower: &str) -> Option<String> {
    let ok = |v: &str| {
        let v = v.trim_start();
        v.starts_with('#')
            || ["png", "jpeg", "jpg", "gif", "webp"]
                .iter()
                .any(|t| v.starts_with(&format!("data:image/{t};base64,")))
    };
    let mut from = 0;
    while let Some(k) = lower[from..].find("href") {
        let at = from + k + 4;
        let rest = lower[at..].trim_start();
        if let Some(rest) = rest.strip_prefix('=') {
            let rest = rest.trim_start();
            let v = rest.trim_start_matches(['"', '\'']);
            if !ok(v) {
                return Some(v.chars().take(40).collect());
            }
        }
        from = at;
    }
    let mut from = 0;
    while let Some(k) = lower[from..].find("url(") {
        let at = from + k + 4;
        let v = lower[at..].trim_start().trim_start_matches(['"', '\'']);
        if !ok(v) {
            return Some(format!("url({})", v.chars().take(40).collect::<String>()));
        }
        from = at;
    }
    None
}

/// An attribute: lowercase name, value, and the byte range of the whole attribute.
type Attr = (String, String, std::ops::Range<usize>);

/// Attributes of the start tag beginning at `start` (`<svg …>`), and the tag's end.
fn root_attributes(s: &str, start: usize) -> Option<(Vec<Attr>, usize)> {
    let b = s.as_bytes();
    let mut i = start + 4;
    let mut out = Vec::new();
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            return None;
        }
        if b[i] == b'>' || (b[i] == b'/' && b.get(i + 1) == Some(&b'>')) {
            return Some((out, i));
        }
        let a0 = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'>' {
            i += 1;
        }
        let name = s.get(a0..i)?.to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if b.get(i) == Some(&b'=') {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            let q = *b.get(i)?;
            if q == b'"' || q == b'\'' {
                let v0 = i + 1;
                let v1 = v0 + s.get(v0..)?.find(q as char)?;
                value = s.get(v0..v1)?.to_string();
                i = v1 + 1;
            } else {
                let v0 = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                    i += 1;
                }
                value = s.get(v0..i)?.to_string();
            }
        }
        out.push((name, value, a0..i));
    }
}

fn length(v: &str) -> Option<f64> {
    let v = v.trim();
    let v = v.strip_suffix("px").unwrap_or(v);
    v.parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0)
}

fn normalize_root(s: &str, start: usize) -> Option<String> {
    let (attrs, end) = root_attributes(s, start)?;
    let get = |n: &str| attrs.iter().find(|a| a.0 == n).map(|a| a.1.as_str());
    let (w, h) = (
        get("width").and_then(length),
        get("height").and_then(length),
    );
    let vb: Vec<f64> = get("viewbox")
        .map(|v| {
            v.split([' ', ','])
                .filter(|x| !x.is_empty())
                .filter_map(|x| x.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    let vb = (vb.len() == 4 && vb[2] > 0.0 && vb[3] > 0.0).then(|| (vb[2], vb[3]));
    let (w, h) = match (w, h, vb) {
        (Some(w), Some(h), _) => (w, h),
        (Some(w), None, Some((vw, vh))) => (w, w * vh / vw),
        (None, Some(h), Some((vw, vh))) => (h * vw / vh, h),
        (None, None, Some(v)) => v,
        _ => (1920.0, 1080.0),
    };
    // Rebuild the start tag without width/height, then add ours.
    let mut tag = String::from("<svg");
    if get("xmlns").is_none() {
        tag.push_str(" xmlns=\"http://www.w3.org/2000/svg\"");
    }
    tag.push_str(&format!(
        " width=\"{}\" height=\"{}\"",
        w.round(),
        h.round()
    ));
    if vb.is_none() && get("viewbox").is_none() {
        tag.push_str(&format!(" viewBox=\"0 0 {} {}\"", w.round(), h.round()));
    }
    for (name, _, range) in &attrs {
        if name != "width" && name != "height" {
            tag.push(' ');
            tag.push_str(s.get(range.clone())?);
        }
    }
    Some(format!("{}{tag}{}", &s[..start], &s[end..]))
}

// --------------------------------------------------------------- images

/// Width and height of a PNG, JPEG or (normalised) SVG.
pub fn image_size(bytes: &[u8]) -> Option<(f64, f64)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 {
        let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        return (w > 0 && h > 0).then(|| (f64::from(w), f64::from(h)));
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xff {
                i += 1;
                continue;
            }
            let m = bytes[i + 1];
            let len = usize::from(u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]));
            if (0xc0..=0xcf).contains(&m) && ![0xc4, 0xc8, 0xcc].contains(&m) {
                let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]);
                let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]);
                return (w > 0 && h > 0).then(|| (f64::from(w), f64::from(h)));
            }
            i += 2 + len.max(2);
        }
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let start = text.to_ascii_lowercase().find("<svg")?;
    let (attrs, _) = root_attributes(text, start)?;
    let get = |n: &str| attrs.iter().find(|a| a.0 == n).and_then(|a| length(&a.1));
    Some((get("width")?, get("height")?))
}

/// An image given as SVG markup or base64 PNG/JPEG: checked bytes and extension.
fn image_from(
    svg: Option<&str>,
    b64: Option<&str>,
    what: &str,
) -> Result<Option<(Vec<u8>, &'static str)>> {
    match (svg, b64) {
        (Some(_), Some(_)) => Err(invalid(format!(
            "give the {what} as SVG or as a base64 image, not both"
        ))),
        (Some(svg), None) => {
            let clean = sanitize_svg(svg).map_err(|e| invalid(format!("{what}: {e}")))?;
            Ok(Some((clean.into_bytes(), "svg")))
        }
        (None, Some(b64)) => {
            let data = b64
                .trim()
                .split_once("base64,")
                .map_or(b64.trim(), |(_, d)| d);
            if data.len() > MAX_IMAGE_BYTES / 3 * 4 + 4 {
                return Err(invalid(format!("{what}: the image is larger than 8 MiB")));
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data.trim())
                .map_err(|e| invalid(format!("{what}: not valid base64 ({e})")))?;
            let ext = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                "png"
            } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
                "jpg"
            } else {
                return Err(invalid(format!(
                    "{what}: only PNG or JPEG bytes are accepted (or pass SVG markup)"
                )));
            };
            if image_size(&bytes).is_none() {
                return Err(invalid(format!("{what}: the image could not be read")));
            }
            Ok(Some((bytes, ext)))
        }
        (None, None) => Ok(None),
    }
}

// ------------------------------------------------------------------ ops

fn fit(s: &str) -> Result<BgFit> {
    match s {
        "fit" | "fill" | "stretch" | "tile" | "center" | "centre" => {
            Ok(BgFit::parse(if s == "centre" { "center" } else { s }))
        }
        _ => Err(invalid(format!(
            "background_fit must be fit, fill, stretch, tile or center, got `{s}`"
        ))),
    }
}

fn unit(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() && (0.0..=1.0).contains(&v) {
        Ok(v)
    } else {
        Err(invalid(format!("{name} must be between 0 and 1, got {v}")))
    }
}

fn background_json(b: &Option<PageBackground>) -> Value {
    match b {
        Some(b) => {
            json!({ "image": format!("assets/{}", b.image), "fit": b.fit.as_str(), "opacity": b.opacity, "dim": b.dim })
        }
        None => Value::Null,
    }
}

fn style_json(s: &PageStyle) -> Value {
    json!({
        "color": color_hex(s.bg_color),
        "pattern": s.pattern.as_str(),
        "scheme": if is_dark(s.bg_color) { "dark" } else { "light" },
        "background_image": background_json(&s.background),
    })
}

pub(crate) fn branding_json(b: &Option<Branding>) -> Value {
    match b {
        None => Value::Null,
        Some(b) if b.off => json!({ "hidden": true }),
        Some(b) => json!({
            "logo": b.logo.as_ref().map(|l| format!("assets/{l}")),
            "logo_x": b.logo_x, "logo_y": b.logo_y, "logo_width": b.logo_w, "logo_opacity": b.logo_opacity,
            "school": b.school, "teacher": b.teacher, "class": b.class, "date": b.date,
            "text_position": match b.text_pos { BrandPos::Top => "top", BrandPos::Bottom => "bottom" },
            "text_align": match b.text_align { BrandAlign::Left => "left", BrandAlign::Center => "center", BrandAlign::Right => "right" },
        }),
    }
}

/// `set_page_design`; with nothing to change, the page's design.
pub(crate) fn run_page_design(ctx: &mut BoardCtx<'_>, op: &Op, msg: &str) -> Result<Outcome> {
    let Op::SetPageDesign(a) = op else {
        unreachable!()
    };
    let PageDesignArgs {
        page,
        all_pages,
        color,
        pattern,
        background_svg,
        background_image,
        background_fit,
        background_opacity,
        background_dim,
        remove_background,
    } = a.as_ref();
    let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
    let mut style = ctx.lesson.page_style(&info.id)?;
    let changes = color.is_some()
        || pattern.is_some()
        || background_svg.is_some()
        || background_image.is_some()
        || background_fit.is_some()
        || background_opacity.is_some()
        || background_dim.is_some()
        || *remove_background;
    if !changes {
        let mut out = style_json(&style);
        out["page"] = json!(idx + 1);
        out["branding"] = branding_json(&ctx.lesson.branding_for(&info));
        return Ok(Outcome::read(out));
    }
    if let Some(c) = color {
        style.bg_color = board_color(c)?;
    }
    if let Some(p) = pattern {
        style.pattern = Pattern::from(*p);
    }
    let fit = background_fit.as_deref().map(fit).transpose()?;
    let opacity = background_opacity
        .map(|v| unit("background_opacity", v))
        .transpose()?;
    if let Some((bytes, ext)) = image_from(
        background_svg.as_deref(),
        background_image.as_deref(),
        "background",
    )? {
        let image = ctx.pkg.add_asset(bytes, ext);
        let prev = style.background.take();
        style.background = Some(PageBackground {
            image,
            // A generated SVG is drawn for the slide: stretch it to the page.
            fit: fit
                .or(prev.as_ref().map(|b| b.fit))
                .unwrap_or(if ext == "svg" {
                    BgFit::Stretch
                } else {
                    BgFit::Fill
                }),
            opacity: opacity.or(prev.as_ref().map(|b| b.opacity)).unwrap_or(1.0),
            dim: background_dim
                .or(prev.as_ref().map(|b| b.dim))
                .unwrap_or(false),
        });
    } else if *remove_background {
        style.background = None;
    } else if fit.is_some() || opacity.is_some() || background_dim.is_some() {
        let Some(b) = style.background.as_mut() else {
            return Err(invalid(
                "this page has no background image; pass background_svg or background_image",
            ));
        };
        if let Some(f) = fit {
            b.fit = f;
        }
        if let Some(o) = opacity {
            b.opacity = o;
        }
        if let Some(d) = background_dim {
            b.dim = *d;
        }
    }
    let ids: Vec<PageId> = if *all_pages {
        ctx.lesson.pages().into_iter().map(|p| p.id).collect()
    } else {
        vec![info.id.clone()]
    };
    tag(ctx.lesson, msg);
    ctx.lesson.set_page_style(&ids, &style)?;
    let mut out = style_json(&style);
    out["page"] = json!(idx + 1);
    out["pages_changed"] = json!(ids.len());
    out["ink_hint"] = json!(if is_dark(style.bg_color) {
        "dark page: use light ink and text (white, yellow, light blue)"
    } else {
        "light page: use dark ink and text"
    });
    Ok(Outcome {
        result: out,
        changed: true,
        show_page: Some(idx),
        select: None,
    })
}

/// Logo position for a corner on a page of `frame`, for a logo `w` wide
/// (fraction of the width) with aspect ratio `aspect` (width / height).
fn corner(c: &str, w: f64, frame: Frame, aspect: f64) -> Result<(f64, f64)> {
    let h = w * frame.w / aspect.max(0.01) / frame.h;
    let (mx, my) = (0.025, 0.035);
    let x = |left: bool| if left { mx } else { (1.0 - mx - w).max(0.0) };
    let y = |top: bool| if top { my } else { (1.0 - my - h).max(0.0) };
    Ok(match c {
        "top-left" => (x(true), y(true)),
        "top-right" => (x(false), y(true)),
        "bottom-left" => (x(true), y(false)),
        "bottom-right" => (x(false), y(false)),
        _ => {
            return Err(invalid(format!(
                "logo_corner must be top-left, top-right, bottom-left or bottom-right, got `{c}`"
            )));
        }
    })
}

/// `set_branding`; with nothing to change, the lesson's and the page's branding.
pub(crate) fn run_branding(ctx: &mut BoardCtx<'_>, op: &Op, msg: &str) -> Result<Outcome> {
    let Op::SetBranding(a) = op else {
        unreachable!()
    };
    let BrandingArgs {
        page,
        scope,
        logo_svg,
        logo_image,
        logo_corner,
        logo_x,
        logo_y,
        logo_width,
        logo_opacity,
        remove_logo,
        school,
        teacher,
        class,
        date,
        text_position,
        text_align,
        hidden,
        remove,
    } = a.as_ref();
    let page_scope = match scope.as_deref().unwrap_or("lesson") {
        "lesson" | "all" => false,
        "page" => true,
        s => return Err(invalid(format!("scope must be lesson or page, got `{s}`"))),
    };
    let (idx, info) = page_ref(ctx.lesson, ctx.page_id, *page)?;
    let read = || {
        json!({
            "page": idx + 1,
            "lesson": branding_json(&ctx.lesson.branding()),
            "this_page": branding_json(&info.branding),
        })
    };
    let changes = logo_svg.is_some()
        || logo_image.is_some()
        || logo_corner.is_some()
        || logo_x.is_some()
        || logo_y.is_some()
        || logo_width.is_some()
        || logo_opacity.is_some()
        || *remove_logo
        || school.is_some()
        || teacher.is_some()
        || class.is_some()
        || date.is_some()
        || text_position.is_some()
        || text_align.is_some()
        || hidden.is_some()
        || *remove;
    if !changes {
        return Ok(Outcome::read(read()));
    }
    let set = |ctx: &mut BoardCtx<'_>, b: Option<&Branding>, on_page: bool| -> Result<()> {
        tag(ctx.lesson, msg);
        if on_page {
            ctx.lesson.set_page_branding(&info.id, b)?;
        } else {
            ctx.lesson.set_branding(b)?;
        }
        Ok(())
    };
    let done = |ctx: &BoardCtx<'_>| {
        let info = ctx.lesson.page(&info.id).ok();
        Outcome {
            result: json!({
                "page": idx + 1,
                "lesson": branding_json(&ctx.lesson.branding()),
                "this_page": info.as_ref().map(|i| branding_json(&i.branding)),
                "shown_on_this_page": info.as_ref().map(|i| branding_json(&ctx.lesson.branding_for(i))),
            }),
            changed: true,
            show_page: Some(idx),
            select: None,
        }
    };
    if *remove {
        set(ctx, None, page_scope)?;
        return Ok(done(ctx));
    }
    if let Some(h) = hidden {
        if *h {
            set(ctx, Some(&Branding::hidden()), true)?;
        } else {
            set(ctx, None, true)?;
        }
        return Ok(done(ctx));
    }
    let lesson_b = ctx.lesson.branding();
    let mut b = if page_scope {
        info.branding
            .clone()
            .filter(|b| !b.off)
            .or(lesson_b)
            .unwrap_or_default()
    } else {
        lesson_b.unwrap_or_default()
    };
    let frame = info.frame.unwrap_or(Frame::SLIDE_16_9);
    let mut aspect = b
        .logo
        .as_ref()
        .and_then(|l| ctx.pkg.assets.get(l))
        .and_then(|bytes| image_size(bytes))
        .map_or(1.0, |(w, h)| w / h);
    let was_corner = ["top-left", "top-right", "bottom-left", "bottom-right"]
        .into_iter()
        .find(|c| {
            corner(c, b.logo_w, frame, aspect)
                .is_ok_and(|(x, y)| (x - b.logo_x).abs() < 0.004 && (y - b.logo_y).abs() < 0.004)
        });
    if let Some((bytes, ext)) = image_from(logo_svg.as_deref(), logo_image.as_deref(), "logo")? {
        aspect = image_size(&bytes).map_or(1.0, |(w, h)| w / h);
        b.logo = Some(ctx.pkg.add_asset(bytes, ext));
    } else if *remove_logo {
        b.logo = None;
    }
    if let Some(w) = logo_width {
        if !(w.is_finite() && (Branding::MIN_LOGO_W..=Branding::MAX_LOGO_W).contains(w)) {
            return Err(invalid(format!(
                "logo_width is a fraction of the page width, 0.02 to 0.6 (0.1 is a small corner logo), got {w}"
            )));
        }
        b.logo_w = *w;
    }
    if let Some(o) = logo_opacity {
        b.logo_opacity = unit("logo_opacity", *o)?;
    }
    // A corner, else explicit fractions; a logo that sat in a corner stays there.
    if let Some(c) = logo_corner
        .as_deref()
        .or(if logo_x.is_none() && logo_y.is_none() {
            was_corner
        } else {
            None
        })
    {
        (b.logo_x, b.logo_y) = corner(c, b.logo_w, frame, aspect)?;
    }
    if let Some(x) = logo_x {
        b.logo_x = unit("logo_x", *x)?;
    }
    if let Some(y) = logo_y {
        b.logo_y = unit("logo_y", *y)?;
    }
    for (field, v) in [
        (&mut b.school, school),
        (&mut b.teacher, teacher),
        (&mut b.class, class),
    ] {
        if let Some(v) = v {
            *field = v.clone();
        }
    }
    if let Some(d) = date {
        b.date = *d;
    }
    if let Some(p) = text_position {
        b.text_pos = match p.as_str() {
            "top" | "header" => BrandPos::Top,
            "bottom" | "footer" => BrandPos::Bottom,
            _ => {
                return Err(invalid(format!(
                    "text_position must be top or bottom, got `{p}`"
                )));
            }
        };
    }
    if let Some(a) = text_align {
        b.text_align = match a.as_str() {
            "left" => BrandAlign::Left,
            "center" | "centre" => BrandAlign::Center,
            "right" => BrandAlign::Right,
            _ => {
                return Err(invalid(format!(
                    "text_align must be left, center or right, got `{a}`"
                )));
            }
        };
    }
    b.off = false;
    set(ctx, Some(&b), page_scope)?;
    Ok(done(ctx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_from_an_agent_is_checked_and_sized() {
        let ok = sanitize_svg(
            r##"<svg viewBox="0 0 100 50"><rect width="100" height="50" fill="url(#g)"/></svg>"##,
        )
        .unwrap();
        assert!(
            ok.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50""#),
            "{ok}"
        );
        assert_eq!(image_size(ok.as_bytes()), Some((100.0, 50.0)));
        let sized = sanitize_svg(r#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" width="40px" height='20' viewBox="0 0 4 2"/>"#).unwrap();
        assert!(
            sized.starts_with(r#"<?xml version="1.0"?><svg width="40" height="20" xmlns="#),
            "{sized}"
        );
        let bare = sanitize_svg("<svg><circle r='5'/></svg>").unwrap();
        assert!(bare.contains(r#"width="1920" height="1080" viewBox="0 0 1920 1080""#));
        for bad in [
            "<svg><script>alert(1)</script></svg>",
            "<svg onload=\"x()\"/>",
            "<svg><a xlink:href=\"javascript:x\"/></svg>",
            "<svg><image href=\"https://example.com/x.png\"/></svg>",
            "<svg><rect style=\"fill:url(http://x/y)\"/></svg>",
            "<svg><style>@import url(x.css)</style></svg>",
            "<svg><circle><animate attributeName=\"r\"/></circle></svg>",
            "<svg><foreignObject><div/></foreignObject></svg>",
            "<!DOCTYPE svg [<!ENTITY x \"y\">]><svg/>",
            "<html><body/></html>",
        ] {
            assert!(sanitize_svg(bad).is_err(), "accepted {bad}");
        }
        assert!(sanitize_svg(&format!("<svg>{}</svg>", "a".repeat(MAX_SVG_BYTES))).is_err());
        // Embedded raster data and in-document references are fine.
        assert!(
            sanitize_svg(
                "<svg><image href=\"data:image/png;base64,AAAA\"/><use href=\"#a\"/></svg>"
            )
            .is_ok()
        );
    }

    #[test]
    fn png_and_jpeg_sizes() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&640u32.to_be_bytes());
        png.extend_from_slice(&480u32.to_be_bytes());
        assert_eq!(image_size(&png), Some((640.0, 480.0)));
        let jpg = [
            0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 11, 8, 0, 120, 0, 160, 3, 0, 0, 0,
        ];
        assert_eq!(image_size(&jpg), Some((160.0, 120.0)));
        assert!(
            image_from(None, Some("aGVsbG8="), "logo").is_err(),
            "not an image"
        );
    }
}
