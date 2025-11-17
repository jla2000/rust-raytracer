#![expect(unexpected_cfgs)]
#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::{UVec2, UVec3, Vec3Swizzles, vec4};
use spirv_std::{Image, spirv};

#[spirv(ray_generation)]
pub fn generate_rays(
    #[spirv(launch_id)] launch_id: UVec3,
    #[spirv(descriptor_set = 0, binding = 0)] output: &Image!(2D, format = rgba8, sampled = false),
) {
    let output_size: UVec2 = output.query_size();

    if launch_id.x < output_size.x && launch_id.y < output_size.y {
        unsafe { output.write(launch_id.xy(), vec4(1.0, 0.0, 1.0, 1.0)) };
    }
}
