@group(0) @binding(0) var main_texture: texture_storage_2d<rgba16float, write>;

struct Immediates {
    data1: vec4<f32>,
    data2: vec4<f32>,
};
var<immediate> constants: Immediates;

@compute @workgroup_size(16, 16, 1)
fn main(
    @builtin(global_invocation_id) global_invocation_id: vec3<u32>, 
    @builtin(local_invocation_id) local_invocation_id: vec3<u32>,
) {
    let texel_coord = vec2(i32(global_invocation_id.x), i32(global_invocation_id.y));
    let size = vec2<i32>(textureDimensions(main_texture));

    let top_color = constants.data1;
    let bottom_color = constants.data2;

    if texel_coord.x < size.x && texel_coord.y < size.y {
        let blend = f32(texel_coord.y) / f32(size.y);
        textureStore(main_texture, texel_coord, mix(top_color, bottom_color, blend));
    }
}
