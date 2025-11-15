#![expect(unexpected_cfgs)]
#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::{Vec2, Vec4, vec2};
use spirv_std::spirv;

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
