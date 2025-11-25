#![expect(unexpected_cfgs)]
#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::{UVec3, Vec3Swizzles, Vec4, vec4};
use spirv_std::{Image, spirv};

#[spirv(ray_generation)]
pub fn generate_rays(
    #[spirv(launch_id)] launch_id: UVec3,
    #[spirv(launch_size)] launch_size: UVec3,
    #[spirv(descriptor_set = 0, binding = 0)] output: &Image!(2D, format = rgba8, sampled = false),
) {
    unsafe {
        output.write(
            launch_id.xy(),
            Vec4::new(
                launch_id.x as f32 / launch_size.x as f32,
                launch_id.y as f32 / launch_size.y as f32,
                0.0,
                1.0,
            ),
        )
    };
}

#[spirv(closest_hit)]
pub fn ray_hit(#[spirv(ray_payload)] payload: &mut Vec4) {
    *payload = vec4(0.0, 1.0, 0.0, 1.0);
}

#[spirv(miss)]
pub fn ray_miss(#[spirv(ray_payload)] payload: &mut Vec4) {
    *payload = vec4(1.0, 0.0, 0.0, 1.0);
}
