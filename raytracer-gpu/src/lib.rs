#![expect(unexpected_cfgs)]
#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::{UVec2, UVec3, Vec2, Vec3Swizzles, Vec4, vec2, vec4};
use spirv_std::{Image, spirv};

#[spirv(vertex)]
pub fn main_vs(
    #[spirv(vertex_index)] vertex_id: i32,
    #[spirv(position)] position: &mut Vec4,
    uv: &mut Vec2,
) {
    *uv = vec2((vertex_id & 1) as f32, ((vertex_id >> 1) & 1) as f32);
    *position = (*uv * 2.0 - 1.0).extend(0.0).extend(1.0);
}

#[spirv(fragment)]
pub fn main_fs(uv: Vec2, output: &mut Vec4) {
    *output = uv.extend(0.0).extend(1.0);
}

#[spirv(compute(threads(10, 10)))]
pub fn main_cs(
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
    #[spirv(descriptor_set = 0, binding = 0)] output: &Image!(
        2D,
        format = rgba8_snorm,
        sampled = false
    ),
) {
    let output_size: UVec2 = output.query_size();

    if global_invocation_id.x < output_size.x && global_invocation_id.y < output_size.y {
        unsafe { output.write(global_invocation_id.xy(), vec4(1.0, 0.0, 0.0, 1.0)) };
    }
}
