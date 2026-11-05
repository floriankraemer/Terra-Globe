struct Frame {
    view_projection: mat4x4<f32>,
};

struct TileUniform {
    // Tile centre minus the camera eye, in meters. Doing this subtraction in
    // f64 on the CPU is what keeps a 40 000 km view jitter-free.
    translation: vec4<f32>,
    // UV sub-rectangle inside the bound texture (ancestor fallback).
    uv_rect: vec4<f32>,
    tint: vec4<f32>,
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
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world = input.position + tile.translation.xyz;
    output.clip_position = frame.view_projection * vec4<f32>(world, 1.0);
    output.uv = tile.uv_rect.xy + input.uv * tile.uv_rect.zw;
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
    return vec4<f32>(linear_to_gamma(sampled.rgb * tile.tint.rgb), 1.0);
}
