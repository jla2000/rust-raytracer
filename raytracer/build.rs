use spirv_builder::{MetadataPrintout, SpirvBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    SpirvBuilder::new("../raytracer-gpu", "spirv-unknown-opengl4.5")
        .print_metadata(MetadataPrintout::Full)
        .build()?;
    Ok(())
}
