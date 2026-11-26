// Marks a rectangle of the frame (set with the scissor) as nearest in the
// depth buffer and writes no colour: ink drawn after it is cut out there.
// The main target uses it to leave the wet tile's area to the tile.

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}
