#![expect(unexpected_cfgs)]
#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::{Mat4, UVec2, UVec3, Vec3Swizzles, vec4};
use spirv_std::{Image, spirv};

pub struct PushConstants {
    num_accumulated_frames: usize,
    inverse_projection: Mat4,
    inverse_view: Mat4,
}

#[spirv(compute(threads(10, 10)))]
pub fn main_cs(
    // #[spirv(push_constant)] push_constants: &PushConstants,
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
    #[spirv(descriptor_set = 0, binding = 0)] output: &Image!(2D, format = rgba8, sampled = false),
) {
    let output_size: UVec2 = output.query_size();

    if global_invocation_id.x < output_size.x && global_invocation_id.y < output_size.y {
        unsafe { output.write(global_invocation_id.xy(), vec4(1.0, 0.0, 1.0, 1.0)) };
    }
}
