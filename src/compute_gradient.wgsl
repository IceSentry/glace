@group(0) @binding(0) var main_texture: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(16, 16, 1)
fn main(
    @builtin(global_invocation_id) global_invocation_id: vec3<u32>, 
    @builtin(local_invocation_id) local_invocation_id: vec3<u32>,
) {
    let texel_coord = vec2(i32(global_invocation_id.x), i32(global_invocation_id.y));
    let size = vec2<i32>(textureDimensions(main_texture));
    if texel_coord.x < size.x && texel_coord.y < size.y {
        var color = vec4(0.0, 0.0, 0.0, 1.0);
        if local_invocation_id.x != 0 && local_invocation_id.y != 0 {
            color.x = f32(texel_coord.x) / f32(size.x);
            color.y = f32(texel_coord.y) / f32(size.y);
        }
        textureStore(main_texture, texel_coord, color);
    }
}
