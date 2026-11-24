// rupok ink shader.
//
// Every stroke segment is one instance. The vertex buffer holds points in
// stroke order with separator points (radius < 0) between strokes, and is
// bound four times at offsets -1, 0, +1, +2 so each instance sees
// (prev, a, b, next). The fragment shader evaluates the union of the three
// capsules prev→a, a→b, b→next, so joints get identical coverage from both
// neighbouring segments; together with a per-stroke depth value and a `Less`
// depth test, translucent strokes never double-blend over themselves.
//
// Brushes (rupok_ink::Brush, ids below) change only how a pixel's coverage
// and colour follow from the points: its distance to the path, the distance
// along the path (`arc`, exact from the points) and its page position (the
// noise texture). Widths that depend on the path (nib, taper) come in the
// point radii. Nothing depends on how densely the stroke is sampled, so
// wet ink and the simplified dry stroke stay pixel-identical
// (docs/performance.md, "The wet/dry contract").

struct Globals {
    cam: vec2<f32>,    // world coordinate at the top-left of the viewport
    scale: f32,        // device pixels per world unit (zoom * dpr)
    _pad0: f32,
    size: vec2<f32>,   // viewport size in device pixels
    _pad1: vec2<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
// Tileable value noise, 256×256, made once (InkRenderer::new): pencil grain
// and chalk.
@group(0) @binding(1) var noise_tex: texture_2d<f32>;
@group(0) @binding(2) var noise_samp: sampler;

const PEN: u32 = 0u;
const BALLPOINT: u32 = 1u;
const FOUNTAIN: u32 = 2u;
const MARKER: u32 = 3u;
const PENCIL: u32 = 4u;
const CHALK: u32 = 5u;
const BRUSH: u32 = 6u;
const DASHED: u32 = 7u;
const DOTTED: u32 = 8u;
const NEON: u32 = 9u;
const RAINBOW: u32 = 10u;
const MAGIC: u32 = 11u;

// Mirrors rupok_ink::brush constants. The fountain pen's and marker's nib
// (width by direction) and the brush's taper are in the point radii
// (rupok_ink::Brush::radii), so here they are plain capsules.
const GLOW_RADII: f32 = 1.6;
const DASH_ON: f32 = 2.0;
const DASH_PERIOD: f32 = 4.0;
const DOT_PERIOD: f32 = 2.0;
const RAINBOW_WIDTHS: f32 = 40.0;
const RAINBOW_MIN: f32 = 160.0;

struct VIn {
    @builtin(vertex_index) vi: u32,
    @location(0) prev: vec3<f32>,
    @location(1) a: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) depth: f32,
    @location(6) arc: f32,
    @location(7) style: vec4<f32>,
    @location(4) b: vec3<f32>,
    @location(5) next: vec3<f32>,
};

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec2<f32>,
    @location(1) @interpolate(flat) a: vec3<f32>,
    @location(2) @interpolate(flat) b: vec3<f32>,
    @location(3) @interpolate(flat) prev: vec3<f32>,
    @location(4) @interpolate(flat) next: vec3<f32>,
    @location(5) @interpolate(flat) color: vec4<f32>,
    // brush id, stroke width, arc length at `a`
    @location(6) @interpolate(flat) info: vec3<f32>,
};

fn min_r(r: f32) -> f32 {
    // Never thinner than ~0.6 device px, so zoomed-out ink stays visible.
    return max(r, 0.6 / g.scale);
}

fn brush_of(style: vec4<f32>) -> u32 {
    return u32(round(style.x * 255.0));
}

fn width_of(style: vec4<f32>) -> f32 {
    return (round(style.y * 255.0) + round(style.z * 255.0) * 256.0) / 64.0;
}

/// How far ink reaches past the point radius (rupok_ink::Brush::reach).
fn reach(k: u32) -> f32 {
    if (k == NEON || k == MAGIC) { return 1.0 + GLOW_RADII; }
    return 1.0;
}

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    o.color = v.color;
    let k = brush_of(v.style);
    o.info = vec3<f32>(f32(k), width_of(v.style), v.arc);
    if (v.a.z < 0.0 || v.b.z < 0.0) {
        // Separator: degenerate, clipped away.
        o.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return o;
    }
    let a = vec3<f32>(v.a.xy, min_r(v.a.z));
    let b = vec3<f32>(v.b.xy, min_r(v.b.z));
    o.a = a;
    o.b = b;
    o.prev = select(vec3<f32>(v.prev.xy, min_r(v.prev.z)), vec3<f32>(0.0, 0.0, -1.0), v.prev.z < 0.0);
    o.next = select(vec3<f32>(v.next.xy, min_r(v.next.z)), vec3<f32>(0.0, 0.0, -1.0), v.next.z < 0.0);

    let d = b.xy - a.xy;
    let len = length(d);
    var dir = vec2<f32>(1.0, 0.0);
    if (len > 1e-6) { dir = d / len; }
    let n = vec2<f32>(-dir.y, dir.x);
    let pad = max(a.z, b.z) * reach(k) + 1.5 / g.scale;

    // Triangle strip: 0=(start,-), 1=(end,-), 2=(start,+), 3=(end,+)
    //
    // The quad reaches `pad` behind `a` (the start cap, and the outer wedge of
    // the joint with the previous segment) but stops at `b` unless this is the
    // stroke's last segment (the end cap). Segments of one stroke share a
    // depth and the first fragment wins, so a quad that reached ahead would
    // claim pixels that belong to segments two or more steps on, which this
    // instance cannot see: dense wet ink (a point every ~0.5 px) came out
    // thinner than the same stroke dry, and thickened at pen-up.
    let along_end = (v.vi & 1u) == 1u;
    let side = select(-1.0, 1.0, (v.vi & 2u) == 2u);
    let ahead = select(0.0, pad, v.next.z < 0.0);
    let base = select(a.xy - dir * pad, b.xy + dir * ahead, along_end);
    let world = base + n * pad * side;
    o.world = world;

    let px = (world - g.cam) * g.scale;
    let ndc = px / g.size * 2.0 - 1.0;
    o.pos = vec4<f32>(ndc.x, -ndc.y, v.depth, 1.0);
    return o;
}

/// Signed distance to the tapered capsule a→b, and where along it (0..1)
/// the nearest point is.
fn capsule(p: vec2<f32>, a: vec3<f32>, b: vec3<f32>) -> vec2<f32> {
    let pa = p - a.xy;
    let ba = b.xy - a.xy;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-12), 0.0, 1.0);
    return vec2<f32>(length(pa - ba * h) - mix(a.z, b.z, h), h);
}

fn noise(uv: vec2<f32>) -> f32 {
    return textureSampleLevel(noise_tex, noise_samp, uv, 0.0).r;
}

/// rupok_ink::brush::rainbow_rgb
fn rainbow(s: f32, width: f32) -> vec3<f32> {
    let period = max(RAINBOW_WIDTHS * width, RAINBOW_MIN);
    let h = fract(s / period) * 6.0;
    let c = 0.92 * 0.78;
    let x = c * (1.0 - abs((h % 2.0) - 1.0));
    var rgb = vec3<f32>(c, 0.0, x);
    if (h < 1.0) { rgb = vec3<f32>(c, x, 0.0); }
    else if (h < 2.0) { rgb = vec3<f32>(x, c, 0.0); }
    else if (h < 3.0) { rgb = vec3<f32>(0.0, c, x); }
    else if (h < 4.0) { rgb = vec3<f32>(0.0, x, c); }
    else if (h < 5.0) { rgb = vec3<f32>(x, 0.0, c); }
    return rgb + vec3<f32>(0.92 - c);
}

@fragment
fn fs_main(i: VOut) -> @location(0) vec4<f32> {
    let k = u32(round(i.info.x));
    let width = i.info.y;
    let p = i.world;
    var rgb = i.color.rgb;
    var alpha = i.color.a;
    var cov = 0.0;

    {
        // The nearest of the three capsules (0 = a→b, 1 = prev→a, 2 = b→next).
        var c = capsule(p, i.a, i.b);
        var seg = 0u;
        if (i.prev.z >= 0.0) {
            let q = capsule(p, i.prev, i.a);
            if (q.x < c.x) { c = q; seg = 1u; }
        }
        if (i.next.z >= 0.0) {
            let q = capsule(p, i.b, i.next);
            if (q.x < c.x) { c = q; seg = 2u; }
        }
        var d = c.x;
        // The plain pen, ballpoint, fountain and marker need nothing more (the
        // common case stays as cheap as before brushes). The others read the
        // radius and, for dashes, dots and the rainbow, the distance along
        // the stroke at the nearest point.
        var r = 0.0;
        var s = 0.0;
        if (k >= PENCIL) {
            let e0 = select(select(i.a, i.prev, seg == 1u), i.b, seg == 2u);
            let e1 = select(select(i.b, i.a, seg == 1u), i.next, seg == 2u);
            r = mix(e0.z, e1.z, c.y);
            if (k == DASHED || k == DOTTED || k == RAINBOW) {
                // arc at the segment's start: a's arc, minus or plus a segment.
                let arc_a = i.info.z;
                var s0 = arc_a;
                if (seg == 1u) { s0 = arc_a - distance(i.prev.xy, i.a.xy); }
                if (seg == 2u) { s0 = arc_a + distance(i.a.xy, i.b.xy); }
                s = s0 + c.y * distance(e0.xy, e1.xy);
            }
        }

        if (k == DASHED || k == DOTTED) {
            // Round-capped dashes (or dots) centred along the path.
            let period = select(DASH_PERIOD, DOT_PERIOD, k == DOTTED) * width;
            let on = select(DASH_ON * width, 0.0, k == DOTTED);
            let u = s - on * 0.5 + period * 0.5;
            let along = abs(u - period * floor(u / period) - period * 0.5);
            let lateral = max(d + r, 0.0);
            d = length(vec2<f32>(max(along - on * 0.5, 0.0), lateral)) - r;
        }
        if (k == CHALK) {
            // A broken edge and dusty body, scaled to the stroke.
            let uv = p / (max(width, 1.0) * 30.0);
            d = d + (noise(uv) - 0.5) * r * 0.75;
            alpha = alpha * mix(0.5, 1.0, smoothstep(0.3, 0.72, noise(uv * 3.7 + 0.31)));
        }
        if (k == PENCIL) {
            // Fine paper grain, a little streaked.
            let grain = noise(vec2<f32>(p.x / 36.0, p.y / 44.0));
            alpha = alpha * 0.9 * mix(0.4, 1.0, smoothstep(0.2, 0.75, grain));
        }
        if (k == RAINBOW) {
            rgb = rainbow(s, width);
        }

        cov = clamp(0.5 - d * g.scale, 0.0, 1.0);
        if (k == BRUSH) {
            // A soft edge about a third of the radius deep.
            cov = clamp((0.5 - d * g.scale) / max(1.0, 0.35 * r * g.scale), 0.0, 1.0);
        }
        if (k == NEON || k == MAGIC) {
            // A light core, the colour, and a glow fading out past the line.
            let core = clamp(-d / max(r, 1e-6), 0.0, 1.0);
            rgb = mix(rgb, vec3<f32>(1.0), 0.7 * core * core * cov);
            let gd = clamp(d / (GLOW_RADII * r), 0.0, 1.0);
            let glow = (1.0 - gd) * (1.0 - gd) * 0.5;
            cov = max(cov, glow);
        }
    }

    let a = alpha * cov;
    if (a <= 0.003) { discard; }
    return vec4<f32>(rgb * a, a);
}
