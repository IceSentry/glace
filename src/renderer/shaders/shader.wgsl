struct CameraUniform {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
}
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
}
@group(0) @binding(1)
var<uniform> light: Light;

struct Material {
    base_color: vec4<f32>,
    alpha: f32,
    gloss: f32,
    specular_color: vec3<f32>,
    flags: u32,
}

let MATERIAL_FLAGS_USE_NORMAL_MAP: u32 = 1u;
let MATERIAL_FLAGS_1: u32 = 2u;
let MATERIAL_FLAGS_2: u32 = 4u;
let MATERIAL_FLAGS_3: u32 = 8u;
let MATERIAL_FLAGS_4: u32 = 16u;
let MATERIAL_FLAGS_5: u32 = 32u;
let MATERIAL_FLAGS_6: u32 = 64u;
let MATERIAL_FLAGS_7: u32 = 128u;
let MATERIAL_FLAGS_8: u32 = 256u;
let MATERIAL_FLAGS_9: u32 = 512u;
let MATERIAL_FLAGS_0: u32 = 1024u;

@group(1) @binding(0)
var<uniform> material: Material;

@group(1) @binding(1)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(2)
var s_diffuse: sampler;

@group(1) @binding(3)
var t_normal: texture_2d<f32>;
@group(1) @binding(4)
var s_normal: sampler;

@group(1) @binding(5)
var t_spec: texture_2d<f32>;
@group(1) @binding(6)
var s_spec: sampler;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
}

struct InstanceInput {
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,
    @location(9) normal_matrix_0: vec3<f32>,
    @location(10) normal_matrix_1: vec3<f32>,
    @location(11) normal_matrix_2: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent_position: vec3<f32>,
    @location(4) tangent_light_position: vec3<f32>,
    @location(5) tangent_view_position: vec3<f32>,
}

fn build_model_matrix(instance: InstanceInput) -> mat4x4<f32> {
    return mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
}

fn build_normal_matrix(instance: InstanceInput) -> mat3x3<f32> {
    return mat3x3<f32>(
        instance.normal_matrix_0,
        instance.normal_matrix_1,
        instance.normal_matrix_2,
    );
}

@vertex
fn vertex(
    vertex: Vertex,
    instance: InstanceInput,
) -> VertexOutput {
    let model_matrix = build_model_matrix(instance);
    let normal_matrix = build_normal_matrix(instance);

    let world_normal = normal_matrix * vertex.normal;
    let world_position = model_matrix * vec4<f32>(vertex.position, 1.0);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * world_position;
    out.world_normal = world_normal;
    out.world_position = world_position;
    out.uv = vertex.uv;

    if ((material.flags & MATERIAL_FLAGS_USE_NORMAL_MAP) != 0u) {
        let world_tangent = normalize(normal_matrix * vertex.tangent);
        let world_bitangent = normalize(normal_matrix * vertex.bitangent);
        let tangent_matrix = transpose(mat3x3<f32>(
            world_tangent,
            world_bitangent,
            world_normal,
        ));

        out.tangent_position = tangent_matrix * world_position.xyz;
        out.tangent_view_position = tangent_matrix * camera.view_pos.xyz;
        out.tangent_light_position = tangent_matrix * light.position;
    }

    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let object_color: vec4<f32> = textureSample(t_diffuse, s_diffuse, in.uv);
    var object_specular: vec4<f32> = textureSample(t_spec, s_spec, in.uv);
    let metallic = object_specular.b;
    object_specular = vec4<f32>(metallic, metallic, metallic, 1.0);
    // object_specular = vec4<f32>(1.0, 1.0, 1.0, 1.0) - object_specular;

    var N: vec3<f32>;
    var L: vec3<f32>;
    var V: vec3<f32>;

    if ((material.flags & MATERIAL_FLAGS_USE_NORMAL_MAP) != 0u) {
        let object_normal: vec4<f32> = textureSample(t_normal, s_normal, in.uv);
        N = object_normal.xyz * 2.0 - 1.0;
        L = normalize(in.tangent_light_position - in.tangent_position);
        V = normalize(in.tangent_view_position - in.tangent_position);
        // return vec4<f32>(0.0, 0.0, 1.0, 1.0);
    } else {
        N = normalize(in.world_normal);
        L = normalize(light.position - in.world_position.xyz);
        V = normalize(camera.view_pos.xyz - in.world_position.xyz);
        // return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    }

    let H = normalize(L + V);

    // TODO load ambient values from uniform buffer
    let ambient_strength = 0.1;

    let diffuse_strength = max(dot(N, L), 0.0);

    var specular_strength = max(dot(N, H), 0.0);
    // Make sure the specular light doesn't go pass the lambertian diffuse light
    // this fixes a small artifact, but introduces very sharp cutoff
    specular_strength = specular_strength * f32(diffuse_strength > 0.0);
    let specular_exp = exp2(material.gloss * 11.0) + 2.0;
    specular_strength = pow(specular_strength, specular_exp);

    let ambient_color = ambient_strength * object_color.rgb * material.base_color.rgb;
    let diffuse_color = diffuse_strength * object_color.rgb * material.base_color.rgb;
    let specular_color = specular_strength * object_specular.rgb * material.specular_color;
    let result = (ambient_color + diffuse_color + specular_color) * light.color;
    // let result = diffuse_color;
    // let result = specular_color;
    // let result = object_color.rgb;
    // let result = material.base_color.rgb;
    // let result = N;

    return vec4<f32>(result, object_color.a);
}
