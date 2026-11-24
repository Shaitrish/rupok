//! Media items: YouTube videos, trusted web embeds, video and audio files
//! (`spec/rupok-format.md`, "Media items"; proposed, pending RFC).
//!
//! A media item is an `image` item whose `asset` is a poster and whose
//! `media` map says what plays ([`rupok_core::MediaData`]). This module is
//! the **allowlist**: the only links a host may put in an iframe are the
//! ones [`parse_link`] accepts, and the iframe `src` is always rebuilt from
//! the parsed pieces by [`embed_url`], never copied from the document. A
//! link on any other site is not embedded; hosts offer to open it in the
//! browser instead ([`open_url`]).
//!
//! Embeds are not Smart Objects. Smart Objects are untrusted code in an
//! opaque-origin sandbox with no network; an embed is a page from one of a
//! handful of named education sites, shown in its own origin.

use rupok_core::MediaData;
use serde::Serialize;

/// Largest video or audio file a host stores in a lesson. Files go inside
/// the `.rpk`, so a lesson with a long video is a big file to share.
pub const MAX_MEDIA_BYTES: usize = 200 << 20;
/// Hosts SHOULD warn the teacher above this size.
pub const WARN_MEDIA_BYTES: usize = 50 << 20;

/// A trusted site whose pages may be embedded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Site {
    /// Stored as the media `site` key (`youtube` for YouTube).
    pub id: &'static str,
    pub name: &'static str,
    /// Origins its embeds load from (for CSP `frame-src` and to check
    /// `postMessage` senders).
    pub frame_origins: &'static [&'static str],
}

pub const SITES: &[Site] = &[
    Site {
        id: "youtube",
        name: "YouTube",
        frame_origins: &["https://www.youtube-nocookie.com"],
    },
    Site {
        id: "phet",
        name: "PhET",
        frame_origins: &["https://phet.colorado.edu"],
    },
    Site {
        id: "geogebra",
        name: "GeoGebra",
        frame_origins: &["https://www.geogebra.org"],
    },
    Site {
        id: "desmos",
        name: "Desmos",
        frame_origins: &["https://www.desmos.com"],
    },
    Site {
        id: "google",
        name: "Google Slides and Docs",
        frame_origins: &["https://docs.google.com"],
    },
    Site {
        id: "wikipedia",
        name: "Wikipedia",
        frame_origins: &["https://*.m.wikipedia.org"],
    },
    Site {
        id: "khan",
        name: "Khan Academy",
        frame_origins: &["https://www.khanacademy.org"],
    },
];

pub fn site(id: &str) -> Option<&'static Site> {
    SITES.iter().find(|s| s.id == id)
}

/// A link the board can embed.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MediaLink {
    /// `youtube` or `web` (the media `type`).
    pub kind: &'static str,
    /// [`Site::id`].
    pub site: &'static str,
    pub site_name: &'static str,
    /// Canonical `https` link: stored as the media `src`, opened by "Open
    /// link", printed in PDF exports.
    pub url: String,
    /// The iframe `src` (for YouTube, without player options; see
    /// [`youtube_player_url`]).
    pub embed: String,
    /// A suggested title (from the link; hosts may look up a better one).
    pub title: String,
    /// A picture the host may fetch for the poster, when the site has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_id: Option<String>,
    /// Start and end times found in the link, seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<f64>,
}

// ------------------------------------------------------------------ URLs

/// The parts of an `http(s)` URL that matter here.
struct Url {
    host: String,
    path: String,
    query: String,
    fragment: String,
}

const MAX_URL: usize = 2048;

fn split_url(input: &str) -> Option<Url> {
    let s = input.trim();
    if s.is_empty() || s.len() > MAX_URL || s.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let lower = s.get(..8).unwrap_or(s).to_ascii_lowercase();
    let rest = if lower.starts_with("https://") {
        &s[8..]
    } else if lower.starts_with("http://") {
        &s[7..]
    } else if lower.contains("://") || s.starts_with("//") {
        return None;
    } else {
        // "youtu.be/…", "www.desmos.com/…": a link without a scheme.
        s
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.contains('@') {
        return None; // user info: never a link we embed
    }
    let host = match authority.rsplit_once(':') {
        Some((h, "443")) | Some((h, "")) => h,
        Some(_) => return None,
        None => authority,
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || !host.contains('.')
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        || host.split('.').any(str::is_empty)
    {
        return None;
    }
    let (before_frag, fragment) = tail.split_once('#').unwrap_or((tail, ""));
    let (path, query) = before_frag.split_once('?').unwrap_or((before_frag, ""));
    let path = if path.is_empty() { "/" } else { path };
    Some(Url {
        host,
        path: clean_path(path)?,
        query: query.to_string(),
        fragment: fragment.to_string(),
    })
}

/// A path made only of URL-safe characters: non-ASCII is percent-encoded,
/// anything that could break out of an attribute or a URL is refused.
fn clean_path(p: &str) -> Option<String> {
    let mut out = String::with_capacity(p.len());
    for c in p.chars() {
        if c.is_ascii_alphanumeric() || "-._~!$&'()*+,;=:@/%".contains(c) {
            out.push(c);
        } else if !c.is_ascii() {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        } else {
            return None;
        }
    }
    // A stray `%` must start an escape.
    let b = out.as_bytes();
    for (i, &x) in b.iter().enumerate() {
        if x == b'%'
            && !(b.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit))
        {
            return None;
        }
    }
    Some(out)
}

fn query_get<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter_map(|kv| kv.split_once('=').or(Some((kv, ""))))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
}

fn segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

fn token(s: &str, min: usize, max: usize, extra: &str) -> bool {
    (min..=max).contains(&s.len())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || extra.contains(c))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(s.get(i + 1..i + 3).unwrap_or(""), 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// "projectile-motion" → "Projectile Motion".
fn humanize(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A YouTube time: `90`, `90s`, `1m30s`, `1h2m3s`, `01:30`, `1:02:03`.
pub fn parse_time(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() || s.len() > 16 {
        return None;
    }
    let t = if s.contains(':') {
        let mut total = 0.0;
        for part in s.split(':') {
            let v: f64 = part
                .parse()
                .ok()
                .filter(|v: &f64| v.is_finite() && *v >= 0.0)?;
            total = total * 60.0 + v;
        }
        total
    } else if s.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        s.parse().ok()?
    } else {
        let mut total = 0.0;
        let mut num = String::new();
        for c in s.chars() {
            match c {
                '0'..='9' | '.' => num.push(c),
                'h' | 'm' | 's' => {
                    let v: f64 = num.parse().ok()?;
                    total += v * match c {
                        'h' => 3600.0,
                        'm' => 60.0,
                        _ => 1.0,
                    };
                    num.clear();
                }
                _ => return None,
            }
        }
        if !num.is_empty() {
            total += num.parse::<f64>().ok()?;
        }
        total
    };
    (t.is_finite() && (0.0..=86_400.0).contains(&t)).then_some(t)
}

// ----------------------------------------------------------------- sites

fn is_youtube_id(s: &str) -> bool {
    s.len() == 11 && token(s, 11, 11, "-_")
}

fn youtube(u: &Url) -> Option<MediaLink> {
    let seg = segments(&u.path);
    let id = match u.host.as_str() {
        "youtu.be" | "www.youtu.be" => seg.first().copied(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com" => {
            match seg.as_slice() {
                ["watch"] => query_get(&u.query, "v"),
                ["shorts" | "embed" | "live" | "v" | "e", id, ..] => Some(*id),
                _ => None,
            }
        }
        "youtube-nocookie.com" | "www.youtube-nocookie.com" => match seg.as_slice() {
            ["embed", id, ..] => Some(*id),
            _ => None,
        },
        _ => None,
    }?;
    if !is_youtube_id(id) {
        return None;
    }
    let frag_t = u.fragment.strip_prefix("t=");
    let start = query_get(&u.query, "t")
        .or_else(|| query_get(&u.query, "start"))
        .or(frag_t)
        .and_then(parse_time)
        .filter(|t| *t > 0.0);
    let end = query_get(&u.query, "end")
        .and_then(parse_time)
        .filter(|e| start.is_none_or(|s| *e > s));
    Some(MediaLink {
        kind: "youtube",
        site: "youtube",
        site_name: "YouTube",
        url: format!("https://www.youtube.com/watch?v={id}"),
        embed: format!("https://www.youtube-nocookie.com/embed/{id}"),
        title: "YouTube video".into(),
        poster: Some(format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg")),
        video_id: Some(id.to_string()),
        start,
        end,
    })
}

fn web(
    site: &'static str,
    url: String,
    embed: String,
    title: String,
    poster: Option<String>,
) -> MediaLink {
    let s = self::site(site).expect("known site");
    MediaLink {
        kind: "web",
        site: s.id,
        site_name: s.name,
        url,
        embed,
        title,
        poster,
        video_id: None,
        start: None,
        end: None,
    }
}

fn phet(u: &Url) -> Option<MediaLink> {
    if u.host != "phet.colorado.edu" {
        return None;
    }
    let slug = |s: &str| token(s, 1, 80, "-");
    let sim = match segments(&u.path).as_slice() {
        ["sims", "html", sim, ver, file]
            if slug(sim)
                && (*ver == "latest" || token(ver, 1, 16, "."))
                && file.ends_with(".html")
                && token(file.trim_end_matches(".html"), 1, 120, "-_") =>
        {
            let link = format!("https://phet.colorado.edu/sims/html/{sim}/{ver}/{file}");
            return Some(web(
                "phet",
                link.clone(),
                link,
                humanize(sim),
                Some(format!(
                    "https://phet.colorado.edu/sims/html/{sim}/latest/{sim}-600.png"
                )),
            ));
        }
        [lang, "simulations" | "simulation", sim, ..] if token(lang, 2, 8, "_-") && slug(sim) => {
            *sim
        }
        ["simulations" | "simulation", sim, ..] if slug(sim) => *sim,
        _ => return None,
    };
    let link = format!("https://phet.colorado.edu/sims/html/{sim}/latest/{sim}_all.html");
    Some(web(
        "phet",
        link.clone(),
        link,
        humanize(sim),
        Some(format!(
            "https://phet.colorado.edu/sims/html/{sim}/latest/{sim}-600.png"
        )),
    ))
}

const GEOGEBRA_APPS: &[&str] = &[
    "calculator",
    "graphing",
    "geometry",
    "3d",
    "scientific",
    "classic",
    "cas",
    "probability",
    "notes",
];

fn geogebra(u: &Url) -> Option<MediaLink> {
    if u.host != "www.geogebra.org" && u.host != "geogebra.org" {
        return None;
    }
    let id_ok = |s: &str| token(s, 4, 24, "");
    let seg = segments(&u.path);
    let id = match seg.as_slice() {
        ["m", id] if id_ok(id) => Some(*id),
        ["material", "show" | "iframe", "id", id, ..] if id_ok(id) => Some(*id),
        [app, id] if GEOGEBRA_APPS.contains(app) && id_ok(id) => Some(*id),
        [app] if GEOGEBRA_APPS.contains(app) => {
            let link = format!("https://www.geogebra.org/{app}");
            return Some(web(
                "geogebra",
                link.clone(),
                link,
                format!("GeoGebra {}", humanize(app)),
                None,
            ));
        }
        _ => None,
    }?;
    Some(web(
        "geogebra",
        format!("https://www.geogebra.org/m/{id}"),
        format!("https://www.geogebra.org/material/iframe/id/{id}"),
        "GeoGebra activity".into(),
        None,
    ))
}

const DESMOS_APPS: &[&str] = &[
    "calculator",
    "geometry",
    "3d",
    "scientific",
    "fourfunction",
    "matrix",
];

fn desmos(u: &Url) -> Option<MediaLink> {
    if u.host != "www.desmos.com" && u.host != "desmos.com" {
        return None;
    }
    let seg = segments(&u.path);
    let (app, id) = match seg.as_slice() {
        [app] if DESMOS_APPS.contains(app) => (*app, None),
        [app, id] if DESMOS_APPS.contains(app) && token(id, 4, 32, "") => (*app, Some(*id)),
        // Localised paths: /fr/calculator/abc
        [lang, app, rest @ ..]
            if token(lang, 2, 5, "-") && DESMOS_APPS.contains(app) && rest.len() <= 1 =>
        {
            match rest {
                [] => (*app, None),
                [id] if token(id, 4, 32, "") => (*app, Some(*id)),
                _ => return None,
            }
        }
        _ => return None,
    };
    let path = match id {
        Some(id) => format!("{app}/{id}"),
        None => app.to_string(),
    };
    let link = format!("https://www.desmos.com/{path}");
    Some(web(
        "desmos",
        link.clone(),
        format!("{link}?embed"),
        format!(
            "Desmos {}",
            if app == "3d" {
                "3D".into()
            } else {
                humanize(app)
            }
        ),
        None,
    ))
}

fn google(u: &Url) -> Option<MediaLink> {
    if u.host != "docs.google.com" {
        return None;
    }
    let id_ok = |s: &str| token(s, 10, 200, "-_");
    let seg = segments(&u.path);
    // Drop a signed-in account prefix: /u/0/presentation/...
    let seg: &[&str] = match seg.as_slice() {
        ["u", n, rest @ ..] if token(n, 1, 2, "") => rest,
        s => s,
    };
    let (url, embed, title) = match seg {
        ["presentation", "d", "e", id, ..] if id_ok(id) => (
            format!("https://docs.google.com/presentation/d/e/{id}/pub"),
            format!("https://docs.google.com/presentation/d/e/{id}/embed?start=false&loop=false"),
            "Google Slides",
        ),
        ["presentation", "d", id, ..] if id_ok(id) => (
            format!("https://docs.google.com/presentation/d/{id}/preview"),
            format!("https://docs.google.com/presentation/d/{id}/embed?start=false&loop=false"),
            "Google Slides",
        ),
        ["document", "d", "e", id, ..] if id_ok(id) => (
            format!("https://docs.google.com/document/d/e/{id}/pub"),
            format!("https://docs.google.com/document/d/e/{id}/pub?embedded=true"),
            "Google Doc",
        ),
        ["document", "d", id, ..] if id_ok(id) => (
            format!("https://docs.google.com/document/d/{id}/preview"),
            format!("https://docs.google.com/document/d/{id}/preview"),
            "Google Doc",
        ),
        ["spreadsheets", "d", "e", id, ..] if id_ok(id) => (
            format!("https://docs.google.com/spreadsheets/d/e/{id}/pubhtml"),
            format!(
                "https://docs.google.com/spreadsheets/d/e/{id}/pubhtml?widget=true&headers=false"
            ),
            "Google Sheet",
        ),
        ["forms", "d", "e", id, ..] if id_ok(id) => (
            format!("https://docs.google.com/forms/d/e/{id}/viewform"),
            format!("https://docs.google.com/forms/d/e/{id}/viewform?embedded=true"),
            "Google Form",
        ),
        _ => return None,
    };
    Some(web("google", url, embed, title.into(), None))
}

fn wikipedia(u: &Url) -> Option<MediaLink> {
    let lang = u
        .host
        .strip_suffix(".m.wikipedia.org")
        .or_else(|| u.host.strip_suffix(".wikipedia.org"))?;
    if !token(lang, 2, 20, "-") {
        return None;
    }
    let article = u.path.strip_prefix("/wiki/")?;
    if article.is_empty() || article.len() > 512 {
        return None;
    }
    let title = percent_decode(article).replace('_', " ");
    Some(web(
        "wikipedia",
        format!("https://{lang}.wikipedia.org/wiki/{article}"),
        format!("https://{lang}.m.wikipedia.org/wiki/{article}"),
        format!("{} (Wikipedia)", title.trim()),
        None,
    ))
}

fn khan(u: &Url) -> Option<MediaLink> {
    if u.host != "www.khanacademy.org" && u.host != "khanacademy.org" {
        return None;
    }
    if u.path == "/" || u.path.len() > 512 {
        return None;
    }
    let last = segments(&u.path).last().copied().unwrap_or("");
    let link = format!("https://www.khanacademy.org{}", u.path);
    Some(web(
        "khan",
        link.clone(),
        link,
        format!("{} (Khan Academy)", humanize(&percent_decode(last))),
        None,
    ))
}

/// Recognise a link the board may embed. `None` for anything not on the
/// allowlist (or malformed): offer to open it in the browser instead.
pub fn parse_link(input: &str) -> Option<MediaLink> {
    let u = split_url(input)?;
    youtube(&u)
        .or_else(|| phet(&u))
        .or_else(|| geogebra(&u))
        .or_else(|| desmos(&u))
        .or_else(|| google(&u))
        .or_else(|| wikipedia(&u))
        .or_else(|| khan(&u))
}

/// An `https` link for "Open in browser": any well-formed `http(s)` link,
/// normalised. `None` for other schemes (`javascript:`, `file:`, `data:`…).
pub fn open_url(input: &str) -> Option<String> {
    let s = input.trim();
    let lower = s.get(..8).unwrap_or(s).to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return None;
    }
    let u = split_url(s)?;
    let scheme = if lower.starts_with("https://") {
        "https"
    } else {
        "http"
    };
    let mut out = format!("{scheme}://{}{}", u.host, u.path);
    if !u.query.is_empty() && clean_path(&u.query).is_some() {
        out.push('?');
        out.push_str(&clean_path(&u.query)?);
    }
    Some(out)
}

/// The link a stored media item opens in a browser: its `src` when that is
/// a link on the allowlist, re-derived through [`parse_link`].
pub fn media_link(media: &MediaData) -> Option<MediaLink> {
    match media.kind.as_str() {
        "youtube" | "web" => parse_link(&media.src).filter(|l| l.kind == media.kind),
        _ => None,
    }
}

/// The iframe `src` for a stored media item, or `None` when its link is not
/// (or no longer) on the allowlist. `origin` is the host page's origin,
/// passed to YouTube for its player API.
pub fn embed_url(media: &MediaData, origin: Option<&str>) -> Option<String> {
    let link = media_link(media)?;
    match link.kind {
        "youtube" => Some(youtube_player_url(link.video_id.as_deref()?, media, origin)),
        _ => Some(link.embed),
    }
}

/// `https://www.youtube-nocookie.com/embed/<id>?…` with the item's start,
/// end, mute, loop and autoplay, and the JS API on (so the host can play
/// and pause with `postMessage`).
pub fn youtube_player_url(id: &str, media: &MediaData, origin: Option<&str>) -> String {
    let mut q = vec![
        "enablejsapi=1".to_string(),
        "playsinline=1".into(),
        "rel=0".into(),
        "modestbranding=1".into(),
    ];
    if let Some(s) = media.start.filter(|s| *s > 0.0) {
        q.push(format!("start={}", s.floor() as u64));
    }
    if let Some(e) = media.end {
        q.push(format!("end={}", e.ceil() as u64));
    }
    if media.muted {
        q.push("mute=1".into());
    }
    if media.looped {
        q.push(format!("loop=1&playlist={id}"));
    }
    if media.autoplay {
        q.push("autoplay=1".into());
    }
    if let Some(o) = origin.filter(|o| is_web_origin(o)) {
        let enc: String = o
            .bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect();
        q.push(format!("origin={enc}"));
    }
    format!(
        "https://www.youtube-nocookie.com/embed/{id}?{}",
        q.join("&")
    )
}

/// `http(s)://host[:port]`, nothing else (the page origin for YouTube).
fn is_web_origin(o: &str) -> bool {
    let Some(rest) = o
        .strip_prefix("https://")
        .or_else(|| o.strip_prefix("http://"))
    else {
        return false;
    };
    let (host, port) = rest.split_once(':').unwrap_or((rest, "1"));
    !host.is_empty()
        && host.len() <= 253
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        && token(port, 1, 5, "")
        && port.bytes().all(|b| b.is_ascii_digit())
}

/// The kind of a video or audio file by its first bytes (`video` or
/// `audio`) and a file extension for it, or `None` for anything else.
/// Sniffed, never trusted from the name (spec: Assets).
pub fn sniff_media(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    let at = |i: usize, s: &[u8]| bytes.get(i..i + s.len()) == Some(s);
    if at(4, b"ftyp") {
        // ISO BMFF: M4A / M4B audio brands, everything else is video.
        let brand = bytes.get(8..12).unwrap_or(b"");
        return Some(if brand == b"M4A " || brand == b"M4B " {
            ("audio", "m4a")
        } else {
            ("video", "mp4")
        });
    }
    if at(0, &[0x1a, 0x45, 0xdf, 0xa3]) {
        return Some(("video", "webm"));
    }
    if at(0, b"OggS") {
        return Some(("audio", "ogg"));
    }
    if at(0, b"ID3") || (bytes.len() > 1 && bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0) {
        return Some(("audio", "mp3"));
    }
    if at(0, b"RIFF") && at(8, b"WAVE") {
        return Some(("audio", "wav"));
    }
    None
}

/// A plain poster card (SVG) for a media item that has no picture, e.g.
/// one an agent added: the kind, the title and the site, in the board's
/// flat mint style. Hosts with a canvas may make a nicer one.
pub fn poster_svg(kind: &str, title: &str, caption: &str) -> String {
    fn esc(s: &str) -> String {
        let s: String = s.chars().filter(|c| !c.is_control()).take(80).collect();
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
    let glyph = match kind {
        "audio" => {
            r##"<path d="M610 300v130a40 40 0 1 1-20-34V320l100-20v100a40 40 0 1 1-20-34V300z" fill="#04241b"/>"##
        }
        "web" => {
            r##"<circle cx="640" cy="340" r="60" fill="none" stroke="#04241b" stroke-width="12"/><path d="M580 340h120M640 280c-30 30-30 90 0 120M640 280c30 30 30 90 0 120" fill="none" stroke="#04241b" stroke-width="10"/>"##
        }
        _ => r##"<path d="M618 300l70 40-70 40z" fill="#04241b"/>"##,
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720"><rect width="1280" height="720" fill="#e6fcf5"/><rect x="24" y="24" width="1232" height="672" rx="28" fill="none" stroke="#0ca678" stroke-width="4" stroke-dasharray="14 12"/><circle cx="640" cy="340" r="96" fill="#3ee6b0"/>{glyph}<text x="640" y="520" text-anchor="middle" font-family="Figtree, Inter, system-ui, sans-serif" font-size="44" font-weight="700" fill="#111418">{}</text><text x="640" y="574" text-anchor="middle" font-family="Figtree, Inter, system-ui, sans-serif" font-size="28" fill="#087f5b">{}</text></svg>"##,
        esc(title),
        esc(caption)
    )
}

/// What an agent or a tool asks to place: a link, or a video or audio file.
#[derive(Clone, Debug, Default)]
pub struct MediaInput {
    pub url: Option<String>,
    pub file: Option<Vec<u8>>,
    /// The file's name, for the default title.
    pub file_name: Option<String>,
    pub title: Option<String>,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub muted: bool,
    pub looped: bool,
    pub autoplay: bool,
}

/// Turn a request into a checked `media` map plus a poster, adding the clip
/// and the poster (a plain SVG card) to the package. Returns the media and
/// the poster's asset name. Errors explain what to do instead.
pub fn prepare(pkg: &mut crate::Package, input: MediaInput) -> Result<(MediaData, String), String> {
    let mut media = MediaData {
        start: input.start,
        end: input.end,
        muted: input.muted,
        looped: input.looped,
        autoplay: input.autoplay,
        ..MediaData::default()
    };
    let caption: String = match (input.url, input.file) {
        (Some(url), None) => {
            let link = parse_link(&url).ok_or_else(|| {
                let names: Vec<&str> = SITES.iter().map(|s| s.name).collect();
                format!(
                    "`{url}` is not a link Rupok can embed. Embeds come only from: {}. Share other links as text.",
                    names.join(", ")
                )
            })?;
            media.kind = link.kind.into();
            media.site = link.site.into();
            media.src = link.url;
            media.start = media.start.or(link.start);
            media.end = media.end.or(link.end);
            media.title = input.title.unwrap_or(link.title);
            link.site_name.to_string()
        }
        (None, Some(bytes)) => {
            if bytes.len() > MAX_MEDIA_BYTES {
                return Err(format!(
                    "the file is too big (limit {} MB)",
                    MAX_MEDIA_BYTES >> 20
                ));
            }
            let (kind, ext) = sniff_media(&bytes)
                .ok_or("not a video or audio file (use mp4, webm, mp3, m4a, ogg or wav)")?;
            media.kind = kind.into();
            media.title = input.title.unwrap_or_else(|| {
                input
                    .file_name
                    .as_deref()
                    .map(crate::model3d::title_from_name)
                    .unwrap_or_else(|| if kind == "audio" { "Audio" } else { "Video" }.into())
            });
            media.src = pkg.add_asset(bytes, ext);
            if kind == "audio" { "Audio" } else { "Video" }.into()
        }
        _ => return Err("give either a link or a file".into()),
    };
    media.title = media
        .title
        .chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect();
    if media
        .end
        .is_some_and(|e| media.start.is_some_and(|s| e <= s))
    {
        return Err("`end` must be after `start`".into());
    }
    if !media.is_valid() {
        return Err("invalid start or end time".into());
    }
    let poster = poster_svg(&media.kind, &media.title, &caption);
    let asset = pkg.add_asset(poster.into_bytes(), "svg");
    Ok((media, asset))
}

/// Decode standard base64 (with or without padding; whitespace ignored).
pub fn decode_base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0;
    for b in s.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yt(url: &str) -> MediaLink {
        parse_link(url).unwrap_or_else(|| panic!("not recognised: {url}"))
    }

    #[test]
    fn youtube_links() {
        for url in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "http://youtube.com/watch?feature=share&v=dQw4w9WgXcQ",
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ&list=PL1",
            "https://youtu.be/dQw4w9WgXcQ",
            "youtu.be/dQw4w9WgXcQ",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://www.youtube.com/embed/dQw4w9WgXcQ?rel=0",
            "https://www.youtube.com/live/dQw4w9WgXcQ",
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
            "  https://WWW.YOUTUBE.COM/watch?v=dQw4w9WgXcQ  ",
        ] {
            let l = yt(url);
            assert_eq!(l.kind, "youtube");
            assert_eq!(l.url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
            assert_eq!(
                l.embed,
                "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ"
            );
            assert_eq!(l.video_id.as_deref(), Some("dQw4w9WgXcQ"));
        }
        assert_eq!(yt("https://youtu.be/dQw4w9WgXcQ?t=90").start, Some(90.0));
        assert_eq!(yt("https://youtu.be/dQw4w9WgXcQ?t=1m30s").start, Some(90.0));
        assert_eq!(
            yt("https://www.youtube.com/watch?v=dQw4w9WgXcQ#t=1h0m5s").start,
            Some(3605.0)
        );
        let l = yt("https://www.youtube.com/embed/dQw4w9WgXcQ?start=10&end=25");
        assert_eq!((l.start, l.end), (Some(10.0), Some(25.0)));
        // An end before the start is dropped.
        assert_eq!(
            yt("https://www.youtube.com/embed/dQw4w9WgXcQ?start=30&end=5").end,
            None
        );
    }

    #[test]
    fn rejects_lookalikes_and_bad_ids() {
        for url in [
            "https://www.youtube.com.evil.example/watch?v=dQw4w9WgXcQ",
            "https://evilyoutube.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=short",
            "https://www.youtube.com/watch?v=dQw4w9WgXc\"",
            "https://youtu.be/dQw4w9WgXcQ\"><script>",
            "https://user@www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com:8080/watch?v=dQw4w9WgXcQ",
            "javascript:alert(1)//youtu.be/dQw4w9WgXcQ",
            "ftp://youtu.be/dQw4w9WgXcQ",
            "data:text/html,youtu.be",
            "https://example.com/",
            "https://phet.colorado.edu.evil.com/sims/html/a/latest/a_all.html",
            "https://en.wikipedia.org/w/index.php?title=X",
            "https://docs.google.com/spreadsheets/d/abc",
            "",
        ] {
            assert_eq!(parse_link(url), None, "{url}");
        }
    }

    #[test]
    fn trusted_education_sites() {
        let l = yt("https://phet.colorado.edu/en/simulations/projectile-motion");
        assert_eq!((l.kind, l.site), ("web", "phet"));
        assert_eq!(
            l.embed,
            "https://phet.colorado.edu/sims/html/projectile-motion/latest/projectile-motion_all.html"
        );
        assert_eq!(l.title, "Projectile Motion");
        assert!(
            l.poster
                .as_deref()
                .unwrap()
                .ends_with("projectile-motion-600.png")
        );
        let l =
            yt("https://phet.colorado.edu/sims/html/build-an-atom/latest/build-an-atom_en.html");
        assert_eq!(l.url, l.embed);

        let l = yt("https://www.geogebra.org/m/fNt6aPeJ");
        assert_eq!(
            l.embed,
            "https://www.geogebra.org/material/iframe/id/fNt6aPeJ"
        );
        assert_eq!(
            yt("https://geogebra.org/calculator").embed,
            "https://www.geogebra.org/calculator"
        );

        let l = yt("https://www.desmos.com/calculator/zukjgk9iry");
        assert_eq!(
            l.embed,
            "https://www.desmos.com/calculator/zukjgk9iry?embed"
        );
        assert_eq!(yt("https://www.desmos.com/3d").title, "Desmos 3D");

        let l =
            yt("https://docs.google.com/presentation/d/e/2PACX-1vQabcdefghijkl/pub?start=false");
        assert_eq!(l.site, "google");
        assert!(l.embed.contains("/embed?"));
        assert!(
            yt("https://docs.google.com/u/0/document/d/e/2PACX-1vQabcdefghijkl/pub")
                .embed
                .ends_with("embedded=true")
        );

        let l = yt("https://en.wikipedia.org/wiki/Photosynthesis");
        assert_eq!(l.embed, "https://en.m.wikipedia.org/wiki/Photosynthesis");
        assert_eq!(l.title, "Photosynthesis (Wikipedia)");
        let l = yt("https://bn.wikipedia.org/wiki/সালোকসংশ্লেষণ");
        assert!(l.url.starts_with("https://bn.wikipedia.org/wiki/%E0%A6"));
        assert!(l.title.starts_with("সালোকসংশ্লেষণ"));

        let l = yt("https://www.khanacademy.org/science/biology/photosynthesis-in-plants");
        assert_eq!(l.site, "khan");
        assert_eq!(l.title, "Photosynthesis In Plants (Khan Academy)");
    }

    #[test]
    fn embed_is_rebuilt_from_the_allowlist() {
        let mut m = MediaData {
            kind: "youtube".into(),
            src: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".into(),
            start: Some(30.4),
            end: Some(60.2),
            muted: true,
            looped: true,
            autoplay: true,
            ..MediaData::default()
        };
        let e = embed_url(&m, Some("http://127.0.0.1:3801")).unwrap();
        assert!(e.starts_with("https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?enablejsapi=1"));
        for p in [
            "start=30",
            "end=61",
            "mute=1",
            "loop=1&playlist=dQw4w9WgXcQ",
            "autoplay=1",
            "origin=http%3A%2F%2F127.0.0.1%3A3801",
        ] {
            assert!(e.contains(p), "{p} in {e}");
        }
        // A stored link that is not on the allowlist never becomes a frame.
        m.src = "https://evil.example/watch?v=dQw4w9WgXcQ".into();
        assert_eq!(embed_url(&m, None), None);
        // A web link stored as `youtube` (or the reverse) is refused too.
        m.src = "https://www.desmos.com/calculator".into();
        assert_eq!(embed_url(&m, None), None);
        m.kind = "web".into();
        assert_eq!(
            embed_url(&m, None).as_deref(),
            Some("https://www.desmos.com/calculator?embed")
        );
        m.kind = "video".into();
        assert_eq!(embed_url(&m, None), None);
    }

    #[test]
    fn open_url_only_for_http() {
        assert_eq!(open_url("https://example.com/a b"), None);
        assert_eq!(
            open_url("https://Example.com/x?y=1#frag").as_deref(),
            Some("https://example.com/x?y=1")
        );
        assert_eq!(open_url("javascript:alert(1)"), None);
        assert_eq!(open_url("file:///etc/passwd"), None);
        assert_eq!(open_url("example.com"), None);
    }

    #[test]
    fn times() {
        assert_eq!(parse_time("90"), Some(90.0));
        assert_eq!(parse_time("1:30"), Some(90.0));
        assert_eq!(parse_time("1:02:03"), Some(3723.0));
        assert_eq!(parse_time("2m"), Some(120.0));
        assert_eq!(parse_time("x"), None);
        assert_eq!(parse_time("999999999"), None);
    }

    #[test]
    fn sniffs_media_files() {
        let mut mp4 = vec![0, 0, 0, 0x18];
        mp4.extend_from_slice(b"ftypisom");
        assert_eq!(sniff_media(&mp4), Some(("video", "mp4")));
        let mut m4a = vec![0, 0, 0, 0x18];
        m4a.extend_from_slice(b"ftypM4A ");
        assert_eq!(sniff_media(&m4a), Some(("audio", "m4a")));
        assert_eq!(
            sniff_media(&[0x1a, 0x45, 0xdf, 0xa3, 1]),
            Some(("video", "webm"))
        );
        assert_eq!(sniff_media(b"OggS\0"), Some(("audio", "ogg")));
        assert_eq!(sniff_media(b"ID3\x04"), Some(("audio", "mp3")));
        assert_eq!(sniff_media(b"<html>"), None);
        assert_eq!(sniff_media(b""), None);
    }

    #[test]
    fn prepare_links_and_files() {
        let mut pkg = crate::Package::default();
        let (m, poster) = prepare(
            &mut pkg,
            MediaInput {
                url: Some("https://youtu.be/dQw4w9WgXcQ?t=5".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((m.kind.as_str(), m.start), ("youtube", Some(5.0)));
        assert!(poster.ends_with(".svg") && pkg.assets.contains_key(&poster));
        let err = prepare(
            &mut pkg,
            MediaInput {
                url: Some("https://example.com/v".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("PhET"), "{err}");
        let mut mp3 = b"ID3".to_vec();
        mp3.extend_from_slice(&[4, 0, 0]);
        let (m, _) = prepare(
            &mut pkg,
            MediaInput {
                file: Some(mp3),
                file_name: Some("teacher_intro.mp3".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            (m.kind.as_str(), m.title.as_str()),
            ("audio", "Teacher intro")
        );
        assert!(pkg.assets.contains_key(&m.src));
        assert!(
            prepare(
                &mut pkg,
                MediaInput {
                    file: Some(b"<html>".to_vec()),
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert_eq!(decode_base64("TWFu TWE=").unwrap(), b"ManMa");
        assert_eq!(decode_base64("**"), None);
    }

    #[test]
    fn poster_escapes_text() {
        let s = poster_svg("youtube", "<b>&\"", "YouTube");
        assert!(s.contains("&lt;b&gt;&amp;&quot;"));
        assert!(!s.contains("<b>"));
    }
}
