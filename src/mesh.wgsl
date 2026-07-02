
struct Vertex {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) uv_x: f32,
    @location(2) normal: vec3<f32>,
    @location(3) uv_y: f32,
    @location(4) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

struct Immediates {
    world_matrix: mat4x4<f32>,
};
var<immediate> constants: Immediates;

@vertex
fn vertex(in_vertex: Vertex) -> VertexOutput {
    let x = f32(i32(in_vertex.vertex_index) - 1);
    let y = f32(i32(in_vertex.vertex_index & 1u) * 2 - 1);
    var out: VertexOutput;
    out.clip_position = constants.world_matrix * vec4(in_vertex.position.xy, 0.0, 1.0);
    out.color = in_vertex.color;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(in.color.rgb, 1.0);
}
