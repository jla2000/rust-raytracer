use spirv_builder::{Capability, MetadataPrintout, SpirvBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    SpirvBuilder::new("../raytracer-gpu", "spirv-unknown-vulkan1.4")
        .print_metadata(MetadataPrintout::Full)
        .extension("SPV_KHR_ray_query")
        .extension("SPV_KHR_ray_tracing")
        .capability(Capability::ImageQuery)
        // .capability(Capability::RayQueryKHR)
        // .capability(Capability::RayTracingKHR)
        .build()?;
    Ok(())
}
