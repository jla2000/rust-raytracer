use spirv_builder::{Capability, MetadataPrintout, SpirvBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    SpirvBuilder::new("../raytracer-gpu", "spirv-unknown-vulkan1.4")
        .print_metadata(MetadataPrintout::Full)
        .capability(Capability::ImageQuery)
        .capability(Capability::StorageImageWriteWithoutFormat)
        .build()?;
    Ok(())
}
