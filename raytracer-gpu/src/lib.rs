#![expect(unexpected_cfgs)]
#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::{UVec3, Vec3, Vec3Swizzles, Vec4, vec3, vec4};
use spirv_std::ray_tracing::{AccelerationStructure, RayFlags};
use spirv_std::{Image, spirv};

#[spirv(ray_generation)]
pub fn generate_rays(
    #[spirv(launch_id)] launch_id: UVec3,
    #[spirv(ray_payload)] color: &mut Vec3,
    #[spirv(descriptor_set = 0, binding = 0)] output: &Image!(2D, format = rgba8, sampled = false),
    #[spirv(descriptor_set = 0, binding = 1)] accel_structure: &AccelerationStructure,
) {
    unsafe {
        accel_structure.trace_ray(
            RayFlags::NONE,
            0,
            0,
            0,
            1,
            vec3(0.0, 0.0, 0.0),
            0.5,
            vec3(0.0, 0.0, 1.0),
            1.0,
            color,
        );
    }

    unsafe { output.write(launch_id.xy(), *color) };
}

#[spirv(closest_hit)]
pub fn ray_hit(#[spirv(ray_payload)] color: &mut Vec4) {
    *color = vec4(0.0, 1.0, 0.0, 1.0);
}

#[spirv(miss)]
pub fn ray_miss(#[spirv(ray_payload)] color: &mut Vec4) {
    *color = vec4(1.0, 0.0, 0.0, 1.0);
}
