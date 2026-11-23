// rupok ink shader.
//
// Every stroke segment is one instance. The vertex buffer holds points in
// stroke order with separator points (radius < 0) between strokes, and is
// bound four times at offsets -1, 0, +1, +2 so each instance sees
// (prev, a, b, next). The fragment shader evaluates the union of the three
// capsules prev→a, a→b, b→next, so joints get identical coverage from both
// neighbouring segments; together with a per-stroke depth value and a `Less`
// depth test, translucent strokes never double-blend over themselves.

struct Globals {
    cam: vec2<f32>,    // world coordinate at the top-left of the viewport
    scale: f32,        // device pixels per world unit (zoom * dpr)
    _pad0: f32,
    size: vec2<f32>,   // viewport size in device pixels
    _pad1: vec2<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

struct VIn {
    @builtin(vertex_index) vi: u32,
    @location(0) prev: vec3<f32>,
    @location(1) a: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) depth: f32,
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
};

fn min_r(r: f32) -> f32 {
    // Never thinner than ~0.6 device px, so zoomed-out ink stays visible.
    return max(r, 0.6 / g.scale);
}

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    o.color = v.color;
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
    let pad = max(a.z, b.z) + 1.5 / g.scale;

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

fn capsule(p: vec2<f32>, a: vec3<f32>, b: vec3<f32>) -> f32 {
    let pa = p - a.xy;
    let ba = b.xy - a.xy;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-12), 0.0, 1.0);
    return length(pa - ba * h) - mix(a.z, b.z, h);
}

@fragment
fn fs_main(i: VOut) -> @location(0) vec4<f32> {
    var d = capsule(i.world, i.a, i.b);
    if (i.prev.z >= 0.0) { d = min(d, capsule(i.world, i.prev, i.a)); }
    if (i.next.z >= 0.0) { d = min(d, capsule(i.world, i.b, i.next)); }
    let cov = clamp(0.5 - d * g.scale, 0.0, 1.0);
    if (cov <= 0.003) { discard; }
    let a = i.color.a * cov;
    return vec4<f32>(i.color.rgb * a, a);
}
