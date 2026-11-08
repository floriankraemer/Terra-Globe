struct Frame {
    view_projection: mat4x4<f32>,
};

struct TileUniform {
    // xyz: tile centre minus the camera eye, in meters. Doing this subtraction
    // in f64 on the CPU is what keeps a 40 000 km view jitter-free.
    // w: fraction of the eye distance the mesh is pulled towards the eye (image
    // overlays draped on terrain; 0 for tiles).
    translation: vec4<f32>,
    // UV sub-rectangle inside the bound texture (ancestor fallback).
    uv_rect: vec4<f32>,
    tint: vec4<f32>,
    // Terrain hillshade: xyz = direction towards the light, w = strength (0 =
    // off). Both in the eye-relative frame, whose axes are ECEF's.
    light: vec4<f32>,
    // Local up at the tile, same frame.
    up: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> tile: TileUniform;
@group(1) @binding(1) var imagery: texture_2d<f32>;
@group(1) @binding(2) var imagery_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    var world = input.position + tile.translation.xyz;
    let distance = length(world);
    world = world - world * (min(distance * tile.translation.w, 2000.0) / max(distance, 1.0));
    output.clip_position = frame.view_projection * vec4<f32>(world, 1.0);
    output.uv = tile.uv_rect.xy + input.uv * tile.uv_rect.zw;
    output.world = world;
    return output;
}

fn linear_to_gamma(color: vec3<f32>) -> vec3<f32> {
    let clamped = max(color, vec3<f32>(0.0));
    let cutoff = clamped < vec3<f32>(0.0031308);
    let lower = clamped * 12.92;
    let higher = vec3<f32>(1.055) * pow(clamped, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(higher, lower, cutoff);
}

// The offscreen target is Rgba8Unorm because that is the format egui can
// register as an image, and egui samples user textures as sRGB-encoded. So
// shade in linear space (sRGB tile textures are linearised on sample) and
// encode here.
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(imagery, imagery_sampler, input.uv);
    // Derivatives need uniform control flow, so they are taken before the
    // strength test.
    let slope_x = dpdx(input.world);
    let slope_y = dpdy(input.world);
    var brightness = 1.0;
    if (tile.light.w > 0.0) {
        var normal = normalize(cross(slope_x, slope_y));
        // Face the eye, which is the origin of this frame.
        if (dot(normal, input.world) > 0.0) {
            normal = -normal;
        }
        let flat_level = dot(tile.up.xyz, tile.light.xyz);
        let lit = clamp(dot(normal, tile.light.xyz) / flat_level, 0.3, 1.25);
        // Near-vertical faces are skirts, not ground: shading them would draw
        // a dark line wherever one shows. A degenerate triangle gives NaN,
        // which `select` keeps out as well.
        let ground = dot(normal, tile.up.xyz) > 0.2;
        brightness = select(1.0, mix(1.0, lit, tile.light.w), lit == lit && ground);
    }
    return vec4<f32>(linear_to_gamma(sampled.rgb * tile.tint.rgb * brightness), 1.0);
}

// Overlay textures are stored premultiplied (see `premultiply_srgb`) so that
// filtering and mipmapping never bleed the colour of transparent texels into
// their neighbours. Un-premultiply, encode, and let the blend state apply the
// straight alpha: the target holds gamma-encoded colour, so the mix happens in
// the same space the base layer was written in.
@fragment
fn fs_overlay(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(imagery, imagery_sampler, input.uv);
    let straight = select(vec3<f32>(0.0), sampled.rgb / sampled.a, sampled.a > 0.0);
    let color = min(straight, vec3<f32>(1.0)) * tile.tint.rgb;
    return vec4<f32>(linear_to_gamma(color), sampled.a * tile.tint.a);
}
