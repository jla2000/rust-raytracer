#![expect(deprecated, reason = "New winit interface sucks")]

async fn run() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let window = event_loop
        .create_window(winit::window::WindowAttributes::default())
        .unwrap();

    let surface = instance.create_surface(&window).unwrap();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await
        .unwrap();

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
            ..Default::default()
        })
        .await
        .unwrap();

    let window_size = window.inner_size();
    let mut surface_config = surface
        .get_default_config(&adapter, window_size.width, window_size.height)
        .unwrap();

    surface.configure(&device, &surface_config);

    let spirv_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        source: wgpu::util::make_spirv(include_bytes!(env!("raytracer_gpu.spv"))),
        label: None,
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &spirv_shader,
            entry_point: Some("main_vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &spirv_shader,
            entry_point: Some("main_fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_config.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    });

    let compute_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 800,
            height: 600,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[wgpu::TextureFormat::Rgba32Float],
    });

    let compute_texture_view = compute_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let compute_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::ReadWrite,
                    format: wgpu::TextureFormat::Rgba32Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            }],
        });

    let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &compute_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&compute_texture_view),
        }],
    });

    let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&compute_bind_group_layout],
        push_constant_ranges: &[],
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&compute_pipeline_layout),
        module: &spirv_shader,
        entry_point: Some("main_cs"),
        compilation_options: Default::default(),
        cache: None,
    });

    let window = &window;
    event_loop
        .run(move |event, active_loop| {
            if let winit::event::Event::WindowEvent { event, .. } = event {
                match event {
                    winit::event::WindowEvent::CloseRequested => active_loop.exit(),
                    winit::event::WindowEvent::RedrawRequested => {
                        match render(
                            &surface,
                            &device,
                            &queue,
                            &render_pipeline,
                            &compute_pipeline,
                            &compute_bind_group,
                        ) {
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                let window_size = window.inner_size();
                                surface_config.width = window_size.width;
                                surface_config.height = window_size.height;
                                surface.configure(&device, &surface_config);
                            }
                            Err(e) => log::error!("{e:?}"),
                            Ok(()) => {}
                        }
                    }
                    _ => {}
                }
            }
        })
        .unwrap();
}

fn render(
    surface: &wgpu::Surface,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    render_pipeline: &wgpu::RenderPipeline,
    compute_pipeline: &wgpu::ComputePipeline,
    compute_bind_group: &wgpu::BindGroup,
) -> Result<(), wgpu::SurfaceError> {
    let surface_texture = surface.get_current_texture().unwrap();
    let surface_view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut render_encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let mut render_pass = render_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &surface_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    render_pass.set_pipeline(render_pipeline);
    render_pass.draw(0..4, 0..1);
    drop(render_pass);

    let mut compute_encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let mut compute_pass =
        compute_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());

    compute_pass.set_bind_group(0, compute_bind_group, &[]);
    compute_pass.set_pipeline(compute_pipeline);
    compute_pass.dispatch_workgroups(800, 600, 1);
    drop(compute_pass);

    let command_buffers = [render_encoder.finish(), compute_encoder.finish()];

    queue.submit(command_buffers);
    surface_texture.present();

    Ok(())
}

fn main() {
    env_logger::init();
    pollster::block_on(run());
}
