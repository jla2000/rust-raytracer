#![expect(deprecated)]

use std::sync::Arc;

use vulkano::{
    VulkanLibrary,
    command_buffer::{
        AutoCommandBufferBuilder,
        allocator::{StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo},
    },
    descriptor_set::{
        DescriptorSet, DescriptorSetWithOffsets, WriteDescriptorSet,
        allocator::{StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo},
    },
    device::{
        Device, DeviceCreateInfo, DeviceExtensions, DeviceFeatures, Queue, QueueCreateInfo,
        QueueFlags,
        physical::{PhysicalDevice, PhysicalDeviceType},
    },
    format::Format,
    image::{Image, ImageUsage, view::ImageView},
    instance::{Instance, InstanceCreateInfo},
    pipeline::{
        ComputePipeline, Pipeline, PipelineBindPoint, PipelineLayout,
        PipelineShaderStageCreateInfo, compute::ComputePipelineCreateInfo,
        layout::PipelineDescriptorSetLayoutCreateInfo,
    },
    shader::{ShaderModule, ShaderModuleCreateInfo, spirv::bytes_to_words},
    swapchain::{
        PresentMode, Surface, Swapchain, SwapchainCreateInfo, SwapchainPresentInfo,
        acquire_next_image,
    },
    sync::{self, GpuFuture},
};
use winit::{
    dpi::PhysicalSize,
    event::{Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowAttributes,
};

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let instance_extensions = Surface::required_extensions(&event_loop).unwrap();

    let library = VulkanLibrary::new().unwrap();
    let instance = Instance::new(
        library,
        InstanceCreateInfo {
            enabled_extensions: instance_extensions,
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

    let device_extensions = DeviceExtensions {
        khr_swapchain: true,
        ..Default::default()
    };

    let (physical_device, queue_family_index) =
        select_physical_device(&instance, &surface, &device_extensions);

    println!("Using GPU: {}", physical_device.properties().device_name);

    let (device, queue) = select_device(physical_device, queue_family_index, device_extensions);
    let (swapchain, images) = create_swapchain(&device, surface.clone(), window.inner_size());
    let compute_pipeline = create_compute_pipeline(&device);

    let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
        device.clone(),
        StandardCommandBufferAllocatorCreateInfo::default(),
    ));

    let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
        device.clone(),
        StandardDescriptorSetAllocatorCreateInfo::default(),
    ));

    let descriptor_set_layout = compute_pipeline.layout().set_layouts().first().unwrap();
    let descriptor_sets: Vec<_> = images
        .iter()
        .map(|image| {
            let image_view = ImageView::new_default(image.clone()).unwrap();

            DescriptorSet::new(
                descriptor_set_allocator.clone(),
                descriptor_set_layout.clone(),
                [WriteDescriptorSet::image_view(0, image_view)],
                [],
            )
            .unwrap()
        })
        .collect();

    event_loop
        .run(|event, active_loop| {
            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                physical_key: PhysicalKey::Code(KeyCode::Escape),
                                ..
                            },
                        ..
                    } => {
                        active_loop.exit();
                    }
                    WindowEvent::RedrawRequested => {
                        render(
                            window.inner_size(),
                            &swapchain,
                            &device,
                            &queue,
                            &compute_pipeline,
                            &descriptor_sets,
                            command_buffer_allocator.clone(),
                        );
                    }
                    _ => {}
                }
            }
        })
        .unwrap();
}

fn render(
    window_size: PhysicalSize<u32>,
    swapchain: &Arc<Swapchain>,
    device: &Arc<Device>,
    queue: &Arc<Queue>,
    compute_pipeline: &Arc<ComputePipeline>,
    compute_descriptor_sets: &[Arc<DescriptorSet>],
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
) {
    let (image_index, _suboptimal, acquire_future) =
        acquire_next_image(swapchain.clone(), None).unwrap();

    let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
        command_buffer_allocator,
        queue.queue_family_index(),
        vulkano::command_buffer::CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    unsafe {
        command_buffer_builder
            .bind_pipeline_compute(compute_pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                compute_pipeline.layout().clone(),
                0,
                DescriptorSetWithOffsets::new(
                    compute_descriptor_sets[image_index as usize].clone(),
                    [],
                ),
            )
            .unwrap()
            .dispatch([window_size.width / 10, window_size.height / 10, 1])
    }
    .unwrap();

    let command_buffer = command_buffer_builder.build().unwrap();

    let future = sync::now(device.clone())
        .join(acquire_future)
        .then_execute(queue.clone(), command_buffer.clone())
        .unwrap()
        .then_swapchain_present(
            queue.clone(),
            SwapchainPresentInfo::swapchain_image_index(swapchain.clone(), image_index),
        )
        .then_signal_fence_and_flush()
        .unwrap();

    future.wait(None).unwrap();
}

fn create_compute_pipeline(device: &Arc<Device>) -> Arc<ComputePipeline> {
    let shader_module = unsafe {
        ShaderModule::new(
            device.clone(),
            ShaderModuleCreateInfo::new(
                &bytes_to_words(include_bytes!(env!("raytracer_gpu.spv"))).unwrap(),
            ),
        )
        .unwrap()
    };

    let compute_stage =
        PipelineShaderStageCreateInfo::new(shader_module.entry_point("main_cs").unwrap());
    let compute_layout = PipelineLayout::new(
        device.clone(),
        PipelineDescriptorSetLayoutCreateInfo::from_stages([&compute_stage])
            .into_pipeline_layout_create_info(device.clone())
            .unwrap(),
    )
    .unwrap();

    ComputePipeline::new(
        device.clone(),
        None,
        ComputePipelineCreateInfo::stage_layout(compute_stage, compute_layout),
    )
    .unwrap()
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
            image_usage: ImageUsage::STORAGE,
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
    device_extensions: DeviceExtensions,
) -> (Arc<Device>, Arc<Queue>) {
    let (device, mut queues) = Device::new(
        physical_device,
        DeviceCreateInfo {
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],
            enabled_extensions: device_extensions,
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
