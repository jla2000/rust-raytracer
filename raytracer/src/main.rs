#![expect(deprecated)]

use std::sync::Arc;

use smallvec::smallvec;
use vulkano::{
    VulkanLibrary,
    command_buffer::{
        AutoCommandBufferBuilder, CopyImageInfo, PrimaryAutoCommandBuffer,
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
    image::{Image, ImageCreateInfo, ImageType, ImageUsage, view::ImageView},
    instance::{Instance, InstanceCreateInfo},
    memory::allocator::{AllocationCreateInfo, StandardMemoryAllocator},
    pipeline::{
        Pipeline, PipelineBindPoint, PipelineLayout, PipelineShaderStageCreateInfo,
        layout::PipelineDescriptorSetLayoutCreateInfo,
        ray_tracing::{
            RayTracingPipeline, RayTracingPipelineCreateInfo, RayTracingShaderGroupCreateInfo,
            ShaderBindingTable,
        },
    },
    shader::{ShaderModule, ShaderModuleCreateInfo, spirv::bytes_to_words},
    swapchain::{
        PresentMode, Surface, SurfaceInfo, Swapchain, SwapchainCreateInfo, SwapchainPresentInfo,
        acquire_next_image,
    },
    sync::{self, GpuFuture},
};
use winit::{
    dpi::PhysicalSize,
    event::{Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    platform::x11::EventLoopBuilderExtX11,
    window::WindowAttributes,
};

fn main() {
    let event_loop = EventLoop::builder().with_x11().build().unwrap();
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
            .create_window(WindowAttributes::default().with_resizable(false))
            .unwrap(),
    );
    let window_size = window.inner_size();

    let surface = Surface::from_window(instance.clone(), window.clone()).unwrap();

    let device_extensions = DeviceExtensions {
        khr_buffer_device_address: true,
        khr_swapchain: true,
        khr_ray_query: true,
        khr_ray_tracing_pipeline: true,
        khr_acceleration_structure: true,
        ..Default::default()
    };

    let (physical_device, queue_family_index) =
        select_physical_device(&instance, &surface, &device_extensions);

    println!("Using GPU: {}", physical_device.properties().device_name);

    let (device, queue) = select_device(physical_device, queue_family_index, device_extensions);
    let (swapchain, swapchain_images) = create_swapchain(&device, surface.clone(), window_size);
    let raytracing_pipeline = create_raytracing_pipeline(&device);

    let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));

    let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
        device.clone(),
        StandardCommandBufferAllocatorCreateInfo::default(),
    ));

    let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
        device.clone(),
        StandardDescriptorSetAllocatorCreateInfo::default(),
    ));

    let shader_binding_table =
        ShaderBindingTable::new(memory_allocator.clone(), &raytracing_pipeline).unwrap();

    let raytracing_image = Image::new(
        memory_allocator,
        ImageCreateInfo {
            image_type: ImageType::Dim2d,
            format: Format::R8G8B8A8_UNORM,
            extent: [window_size.width, window_size.height, 1],
            usage: ImageUsage::STORAGE | ImageUsage::TRANSFER_SRC,
            ..Default::default()
        },
        AllocationCreateInfo::default(),
    )
    .unwrap();
    let raytracing_image_view = ImageView::new_default(raytracing_image.clone()).unwrap();

    let raytracing_descriptor_set_layout =
        raytracing_pipeline.layout().set_layouts().first().unwrap();
    let raytracing_descriptor_set = DescriptorSet::new(
        descriptor_set_allocator.clone(),
        raytracing_descriptor_set_layout.clone(),
        [WriteDescriptorSet::image_view(0, raytracing_image_view)],
        [],
    )
    .unwrap();

    let command_buffers: Vec<_> = swapchain_images
        .into_iter()
        .map(|swapchain_image| {
            record_command_buffer(
                &queue,
                swapchain_image,
                raytracing_image.clone(),
                &raytracing_pipeline,
                &raytracing_descriptor_set,
                &shader_binding_table,
                &command_buffer_allocator,
            )
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
                        render(&swapchain, &device, &queue, &command_buffers);
                    }
                    _ => {}
                }
            }
        })
        .unwrap();
}

fn render(
    swapchain: &Arc<Swapchain>,
    device: &Arc<Device>,
    queue: &Arc<Queue>,
    command_buffers: &[Arc<PrimaryAutoCommandBuffer>],
) {
    let (image_index, _suboptimal, acquire_future) =
        acquire_next_image(swapchain.clone(), None).unwrap();

    let future = sync::now(device.clone())
        .join(acquire_future)
        .then_execute(queue.clone(), command_buffers[image_index as usize].clone())
        .unwrap()
        .then_swapchain_present(
            queue.clone(),
            SwapchainPresentInfo::swapchain_image_index(swapchain.clone(), image_index),
        )
        .then_signal_fence_and_flush()
        .unwrap();

    future.wait(None).unwrap();
}

fn record_command_buffer(
    queue: &Arc<Queue>,
    swapchain_image: Arc<Image>,
    raytracing_image: Arc<Image>,
    raytracing_pipeline: &Arc<RayTracingPipeline>,
    raytracing_descriptor_set: &Arc<DescriptorSet>,
    shader_binding_table: &ShaderBindingTable,
    command_buffer_allocator: &Arc<StandardCommandBufferAllocator>,
) -> Arc<PrimaryAutoCommandBuffer> {
    let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
        command_buffer_allocator.clone(),
        queue.queue_family_index(),
        vulkano::command_buffer::CommandBufferUsage::MultipleSubmit,
    )
    .unwrap();

    command_buffer_builder
        .bind_pipeline_ray_tracing(raytracing_pipeline.clone())
        .unwrap()
        .bind_descriptor_sets(
            PipelineBindPoint::RayTracing,
            raytracing_pipeline.layout().clone(),
            0,
            DescriptorSetWithOffsets::new(raytracing_descriptor_set.clone(), []),
        )
        .unwrap();

    unsafe {
        command_buffer_builder.trace_rays(
            shader_binding_table.addresses().clone(),
            [
                raytracing_image.extent()[0],
                raytracing_image.extent()[1],
                1,
            ],
        )
    }
    .unwrap();

    command_buffer_builder
        .copy_image(CopyImageInfo::images(
            raytracing_image.clone(),
            swapchain_image.clone(),
        ))
        .unwrap();

    command_buffer_builder.build().unwrap()
}

fn create_raytracing_pipeline(device: &Arc<Device>) -> Arc<RayTracingPipeline> {
    let shader_module = unsafe {
        ShaderModule::new(
            device.clone(),
            ShaderModuleCreateInfo::new(
                &bytes_to_words(include_bytes!(env!("raytracer_gpu.spv"))).unwrap(),
            ),
        )
        .unwrap()
    };

    let generate_rays_stage =
        PipelineShaderStageCreateInfo::new(shader_module.entry_point("generate_rays").unwrap());

    let raytracing_pipeline_layout = PipelineLayout::new(
        device.clone(),
        PipelineDescriptorSetLayoutCreateInfo::from_stages([&generate_rays_stage])
            .into_pipeline_layout_create_info(device.clone())
            .unwrap(),
    )
    .unwrap();

    RayTracingPipeline::new(
        device.clone(),
        None,
        RayTracingPipelineCreateInfo {
            stages: smallvec![
                PipelineShaderStageCreateInfo::new(
                    shader_module.entry_point("generate_rays").unwrap()
                ),
                PipelineShaderStageCreateInfo::new(shader_module.entry_point("ray_miss").unwrap()),
                PipelineShaderStageCreateInfo::new(shader_module.entry_point("ray_hit").unwrap()),
            ],
            groups: smallvec![
                // ray generation
                RayTracingShaderGroupCreateInfo::General { general_shader: 0 },
                // miss
                RayTracingShaderGroupCreateInfo::General { general_shader: 1 },
            ],
            ..RayTracingPipelineCreateInfo::layout(raytracing_pipeline_layout)
        },
    )
    .unwrap()
}

fn create_swapchain(
    device: &Arc<Device>,
    surface: Arc<Surface>,
    window_size: PhysicalSize<u32>,
) -> (Arc<Swapchain>, Vec<Arc<Image>>) {
    let surface_caps = device
        .physical_device()
        .surface_capabilities(&surface, Default::default())
        .unwrap();

    let surface_formats = device
        .physical_device()
        .surface_formats(&surface, SurfaceInfo::default())
        .unwrap();

    let (image_format, image_color_space) = surface_formats.first().copied().unwrap();

    Swapchain::new(
        device.clone(),
        surface,
        SwapchainCreateInfo {
            present_mode: PresentMode::Fifo,
            image_usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_DST,
            image_extent: [window_size.width, window_size.height],
            image_format,
            image_color_space,
            min_image_count: surface_caps.min_image_count + 1,
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
                ray_tracing_pipeline: true,
                buffer_device_address: true,
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
        .filter(|physical_device| {
            physical_device
                .supported_extensions()
                .contains(device_extensions)
        })
        .filter_map(|physical_device| {
            physical_device
                .queue_family_properties()
                .iter()
                .enumerate()
                .position(|(queue_family_index, queue)| {
                    queue
                        .queue_flags
                        .contains(QueueFlags::GRAPHICS | QueueFlags::COMPUTE)
                        && physical_device
                            .surface_support(queue_family_index as u32, surface)
                            .unwrap_or(false)
                })
                .map(|q| (physical_device, q as u32))
        })
        .min_by_key(
            |(physical_device, _)| match physical_device.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                _ => 2,
            },
        )
        .expect("no device available")
}
