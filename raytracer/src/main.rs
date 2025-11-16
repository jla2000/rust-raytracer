use std::sync::Arc;

use vulkano::{
    VulkanLibrary,
    command_buffer::{
        AutoCommandBufferBuilder,
        allocator::{StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo},
    },
    device::{
        Device, DeviceCreateInfo, DeviceExtensions, DeviceFeatures, Queue, QueueCreateInfo,
        QueueFlags,
        physical::{PhysicalDevice, PhysicalDeviceType},
    },
    format::Format,
    image::{Image, ImageUsage},
    instance::{Instance, InstanceCreateInfo},
    shader::{ShaderModule, ShaderModuleCreateInfo, spirv::bytes_to_words},
    swapchain::{
        self, AcquireNextImageInfo, PresentMode, Surface, Swapchain, SwapchainCreateInfo,
        SwapchainPresentInfo, acquire_next_image,
    },
    sync::{self, GpuFuture},
};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowAttributes,
};

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    let required_extensions = Surface::required_extensions(&event_loop).unwrap();

    let library = VulkanLibrary::new().unwrap();
    let instance = Instance::new(
        library,
        InstanceCreateInfo {
            enabled_extensions: required_extensions,
            ..Default::default()
        },
    )
    .unwrap();

    let window = Arc::new(
        event_loop
            .create_window(WindowAttributes::default())
            .unwrap(),
    );

    let surface = Surface::from_window(instance.clone(), window.clone()).unwrap();

    let required_device_extensions = DeviceExtensions {
        khr_swapchain: true,
        ..Default::default()
    };

    let (physical_device, queue_family_index) =
        select_physical_device(&instance, &surface, &required_device_extensions);

    println!("Using GPU: {}", physical_device.properties().device_name);

    let (device, queue) = select_device(
        physical_device,
        queue_family_index,
        required_device_extensions,
    );

    let shader_words = bytes_to_words(include_bytes!(env!("raytracer_gpu.spv"))).unwrap();
    let shader = unsafe {
        ShaderModule::new(device.clone(), ShaderModuleCreateInfo::new(&shader_words)).unwrap()
    };

    let (swapchain, images) = create_swapchain(&device, surface.clone(), window.inner_size());

    let allocator = Arc::new(StandardCommandBufferAllocator::new(
        device.clone(),
        StandardCommandBufferAllocatorCreateInfo::default(),
    ));

    let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
        allocator,
        queue_family_index,
        vulkano::command_buffer::CommandBufferUsage::MultipleSubmit,
    )
    .unwrap();

    let command_buffer = command_buffer_builder.build().unwrap();

    event_loop
        .run(|event, active_loop| {
            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::CloseRequested => active_loop.exit(),
                    WindowEvent::RedrawRequested => {
                        let (image_index, suboptimal, acquire_future) =
                            acquire_next_image(swapchain.clone(), None).unwrap();

                        let future = sync::now(device.clone())
                            .join(acquire_future)
                            .then_execute(queue.clone(), command_buffer.clone())
                            .unwrap()
                            .then_swapchain_present(
                                queue.clone(),
                                SwapchainPresentInfo::swapchain_image_index(
                                    swapchain.clone(),
                                    image_index,
                                ),
                            )
                            .then_signal_fence_and_flush()
                            .unwrap();

                        future.wait(None).unwrap();
                    }
                    _ => {}
                }
            }
        })
        .unwrap();
}

fn create_swapchain(
    device: &Arc<Device>,
    surface: Arc<Surface>,
    window_size: PhysicalSize<u32>,
) -> (Arc<Swapchain>, Vec<Arc<Image>>) {
    Swapchain::new(
        device.clone(),
        surface,
        SwapchainCreateInfo {
            present_mode: PresentMode::Fifo,
            image_usage: ImageUsage::COLOR_ATTACHMENT,
            image_extent: [window_size.width, window_size.height],
            image_format: Format::R8G8B8A8_UNORM,
            min_image_count: 3,
            ..Default::default()
        },
    )
    .expect("Failed to create swapchain")
}

fn select_device(
    physical_device: Arc<PhysicalDevice>,
    queue_family_index: u32,
    required_extensions: DeviceExtensions,
) -> (Arc<Device>, Arc<Queue>) {
    let (device, mut queues) = Device::new(
        physical_device,
        DeviceCreateInfo {
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],
            enabled_extensions: required_extensions,
            enabled_features: DeviceFeatures {
                vulkan_memory_model: true,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();

    (device, queues.next().unwrap())
}

fn select_physical_device(
    instance: &Arc<Instance>,
    surface: &Arc<Surface>,
    device_extensions: &DeviceExtensions,
) -> (Arc<PhysicalDevice>, u32) {
    instance
        .enumerate_physical_devices()
        .expect("could not enumerate devices")
        .filter(|p| p.supported_extensions().contains(device_extensions))
        .filter_map(|p| {
            p.queue_family_properties()
                .iter()
                .enumerate()
                .position(|(i, q)| {
                    q.queue_flags
                        .contains(QueueFlags::GRAPHICS | QueueFlags::COMPUTE)
                        && p.surface_support(i as u32, surface).unwrap_or(false)
                })
                .map(|q| (p, q as u32))
        })
        .min_by_key(|(p, _)| match p.properties().device_type {
            PhysicalDeviceType::DiscreteGpu => 0,
            PhysicalDeviceType::IntegratedGpu => 1,
            _ => 2,
        })
        .expect("no device available")
}
