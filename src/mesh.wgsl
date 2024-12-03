struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

struct Vertex {
    position: vec3<f32>,
    uv_x: f32,
    normal: vec3<f32>,
    uv_y: f32,
    color: vec4<f32>,
};

struct VertexBuffer {
    vertices: array<Vertex>,
};

@group(0) @binding(0)
var<storage> vertex_buffer: VertexBuffer;

struct PushConstants {
    world_matrix: mat4x4<f32>,
    vertex_buffer: VertexBuffer,
};

var<push_constant> constants: PushConstants;

@vertex
fn vertex(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    let colors = array<vec3f, 3>(
		vec3(1.0f, 0.0f, 0.0f), //red
		vec3(0.0f, 1.0f, 0.0f), //green
		vec3(00.f, 0.0f, 1.0f), //blue

    );

    let x = f32(i32(in_vertex_index) - 1);
    let y = f32(i32(in_vertex_index & 1u) * 2 - 1);
    var out: VertexOutput;
    out.clip_position = vec4(x, y, 0.0, 1.0);
    out.color = colors[in_vertex_index];
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(in.color, 1.0);
}
