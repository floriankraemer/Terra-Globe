struct Frame {
    view_projection: mat4x4<f32>,
    // x = viewport width in pixels, y = height, z/w unused.
    viewport: vec4<f32>,
};

struct Item {
    // xyz: item centre relative to the camera eye, in meters.
    // w: fraction of the eye distance the item is pulled towards the eye (0 =
    // off), see `pull_to_eye`.
    translation: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> item: Item;

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// Slides a point along its view ray towards the eye: it lands on the same
// pixel but nearer in depth. Over terrain the tile surface is a coarser model
// than the heights lines and fills sample, so without this a slope between two
// grid vertices can swallow them; the clearance it buys grows with distance,
// as the surface's own error does. Capped, so a far-side shape cannot show
// through the limb of a whole-globe view.
fn pull_to_eye(world: vec3<f32>, factor: f32) -> vec3<f32> {
    let distance = length(world);
    let pull = min(distance * factor, 2000.0);
    return world - world * (pull / max(distance, 1.0));
}

// ---- filled polygons -------------------------------------------------------

struct FillInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_fill(input: FillInput) -> VertexOutput {
    var output: VertexOutput;
    let world = pull_to_eye(input.position + item.translation.xyz, item.translation.w);
    output.clip = frame.view_projection * vec4<f32>(world, 1.0);
    output.color = input.color;
    return output;
}

// ---- screen-space-width polylines ------------------------------------------

struct LineInput {
    @location(0) start: vec3<f32>,
    @location(1) end: vec3<f32>,
    @location(2) side: f32,
    @location(3) width: f32,
    @location(4) t: f32,
    @location(5) color: vec4<f32>,
};

@vertex
fn vs_line(input: LineInput) -> VertexOutput {
    var output: VertexOutput;
    let a = frame.view_projection
        * vec4<f32>(pull_to_eye(input.start + item.translation.xyz, item.translation.w), 1.0);
    let b = frame.view_projection
        * vec4<f32>(pull_to_eye(input.end + item.translation.xyz, item.translation.w), 1.0);

    // Segments crossing the camera plane are dropped rather than appearing as
    // wedges across the screen.
    if (a.w <= 0.001 || b.w <= 0.001) {
        output.clip = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        output.color = input.color;
        return output;
    }

    let a_ndc = a.xy / a.w;
    let b_ndc = b.xy / b.w;

    // Work in pixel space so the width is constant at every zoom level.
    let half_viewport = frame.viewport.xy * 0.5;
    var direction = (b_ndc - a_ndc) * half_viewport;
    if (length(direction) < 0.00001) {
        output.clip = a;
        output.color = input.color;
        return output;
    }
    direction = normalize(direction);
    let normal = vec2<f32>(-direction.y, direction.x);
    let offset_px = normal * (input.width * 0.5) * input.side;
    let offset_ndc = offset_px / half_viewport;

    let base = select(a, b, input.t > 0.5);
    output.clip = vec4<f32>(base.xy + offset_ndc * base.w, base.z, base.w);
    output.color = input.color;
    return output;
}

@fragment
fn fs_scene(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
