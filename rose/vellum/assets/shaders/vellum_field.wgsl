// A GPU compute shader that runs Conway's Game of Life as an ambient background.
//
// This is the part WebGL2 cannot do: a compute pass that reads and writes
// arbitrary GPU memory, independent of the rasteriser. Each step reads the
// previous frame from `input` and writes the next state to `output`; the two
// textures are swapped every frame to advance the simulation (see field.rs).
//
// The cell's alive/dead bit is stored in the alpha channel so neighbour counting
// can read it straight back. Dead cells write alpha 0 — fully transparent — so
// only living cells tint the page with `alive_color`.

@group(0) @binding(0) var input: texture_storage_2d<rgba32float, read>;
@group(0) @binding(1) var output: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var<uniform> config: FieldUniforms;

struct FieldUniforms {
    alive_color: vec4<f32>,
}

fn hash(value: u32) -> u32 {
    var state = value;
    state = state ^ 2747636419u;
    state = state * 2654435769u;
    state = state ^ (state >> 16u);
    state = state * 2654435769u;
    state = state ^ (state >> 16u);
    state = state * 2654435769u;
    return state;
}

fn random_float(value: u32) -> f32 {
    return f32(hash(value)) / 4294967295.0;
}

@compute @workgroup_size(8, 8, 1)
fn init(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let location = vec2<i32>(i32(invocation_id.x), i32(invocation_id.y));

    let random_number = random_float((invocation_id.y << 16u) | invocation_id.x);
    // A sparse seed keeps the field calm rather than a dense churn behind text.
    let alive = random_number > 0.88;
    let color = vec4(config.alive_color.rgb, f32(alive));

    textureStore(output, location, color);
}

fn is_alive(location: vec2<i32>, offset_x: i32, offset_y: i32) -> i32 {
    let value: vec4<f32> = textureLoad(input, location + vec2<i32>(offset_x, offset_y));
    return i32(value.a);
}

fn count_alive(location: vec2<i32>) -> i32 {
    return is_alive(location, -1, -1) +
           is_alive(location, -1,  0) +
           is_alive(location, -1,  1) +
           is_alive(location,  0, -1) +
           is_alive(location,  0,  1) +
           is_alive(location,  1, -1) +
           is_alive(location,  1,  0) +
           is_alive(location,  1,  1);
}

@compute @workgroup_size(8, 8, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let location = vec2<i32>(i32(invocation_id.x), i32(invocation_id.y));

    let n_alive = count_alive(location);

    var alive: bool;
    if (n_alive == 3) {
        alive = true;
    } else if (n_alive == 2) {
        alive = bool(is_alive(location, 0, 0));
    } else {
        alive = false;
    }
    let color = vec4(config.alive_color.rgb, f32(alive));

    textureStore(output, location, color);
}
