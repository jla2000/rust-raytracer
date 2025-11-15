const SHADER: &[u8] = include_bytes!(env!("raytracer_gpu.spv"));

fn main() {
    println!("Shader: {SHADER:?}");
}
