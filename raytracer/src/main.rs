#![expect(deprecated, reason = "New winit interface sucks")]

fn main() {
    env_logger::init();

    #[cfg(target_os = "linux")]
    use winit::platform::x11::EventLoopBuilderExtX11;

    #[cfg(target_os = "linux")]
    let event_loop = winit::event_loop::EventLoop::builder()
        .with_x11()
        .build()
        .unwrap();

    #[cfg(target_os = "windows")]
    let event_loop = winit::event_loop::EventLoop::new().unwrap();

    let window = event_loop
        .create_window(winit::window::WindowAttributes::default())
        .unwrap();

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });

    let surface = instance.create_surface(&window).unwrap();

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .unwrap();

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
        ..Default::default()
    }))
    .unwrap();

    let mut surface_config = surface
        .get_default_config(
            &adapter,
            window.inner_size().width,
            window.inner_size().height,
        )
        .unwrap();

    surface.configure(&device, &surface_config);

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        source: wgpu::util::make_spirv(include_bytes!(env!("raytracer_gpu.spv"))),
        label: None,
    });

    let compute_pipeline = create_compute_pipeline(&device, &shader);
    let render_pipeline = create_render_pipeline(&device, &shader, surface_config.format);

    let compute_bind_group_layout = compute_pipeline.get_bind_group_layout(0);
    let render_bind_group_layout = render_pipeline.get_bind_group_layout(0);

    let compute_texture_view =
        create_compute_texture(&device, surface_config.width, surface_config.height);
    let compute_texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

    let mut compute_bind_group =
        create_compute_bind_group(&device, &compute_bind_group_layout, &compute_texture_view);

    let mut render_bind_group = create_render_bind_group(
        &device,
        &render_bind_group_layout,
        &compute_texture_view,
        &compute_texture_sampler,
    );

    let window = &window;
    event_loop
        .run(move |event, active_loop| {
            let mut resize = false;

            if let winit::event::Event::WindowEvent { event, .. } = event {
                match event {
                    winit::event::WindowEvent::CloseRequested => active_loop.exit(),
                    winit::event::WindowEvent::RedrawRequested => match render(
                        &device,
                        &queue,
                        &surface,
                        &render_pipeline,
                        &render_bind_group,
                        &compute_pipeline,
                        &compute_bind_group,
                    ) {
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            resize = true
                        }
                        Err(e) => log::error!("{e:?}"),
                        Ok(()) => {}
                    },
                    winit::event::WindowEvent::Resized(size) => {
                        resize = true;
                        if size.width.is_multiple_of(10) && size.height.is_multiple_of(10) {
                            resize = true;
                        } else {
                            _ = window.request_inner_size(winit::dpi::PhysicalSize {
                                width: size.width - size.width % 10,
                                height: size.height - size.height % 10,
                            });
                        }
                    }
                    _ => {}
                }
            }

            if resize {
                surface_config.width = window.inner_size().width;
                surface_config.height = window.inner_size().height;
                surface.configure(&device, &surface_config);

                let compute_texture_view =
                    create_compute_texture(&device, surface_config.width, surface_config.height);

                compute_bind_group = create_compute_bind_group(
                    &device,
                    &compute_bind_group_layout,
                    &compute_texture_view,
                );

                render_bind_group = create_render_bind_group(
                    &device,
                    &render_bind_group_layout,
                    &compute_texture_view,
                    &compute_texture_sampler,
                );
            }
        })
        .unwrap();
}

fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface: &wgpu::Surface,
    render_pipeline: &wgpu::RenderPipeline,
    render_bind_group: &wgpu::BindGroup,
    compute_pipeline: &wgpu::ComputePipeline,
    compute_bind_group: &wgpu::BindGroup,
) -> Result<(), wgpu::SurfaceError> {
    let surface_texture = surface.get_current_texture()?;
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
                load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    render_pass.set_bind_group(0, render_bind_group, &[]);
    render_pass.set_pipeline(render_pipeline);
    render_pass.draw(0..4, 0..1);
    drop(render_pass);

    let mut compute_encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    let mut compute_pass =
        compute_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());

    compute_pass.set_bind_group(0, compute_bind_group, &[]);
    compute_pass.set_pipeline(compute_pipeline);
    compute_pass.dispatch_workgroups(
        surface_view.texture().width() / 10,
        surface_view.texture().height() / 10,
        1,
    );
    drop(compute_pass);

    let command_buffers = [compute_encoder.finish(), render_encoder.finish()];

    queue.submit(command_buffers);
    surface_texture.present();

    Ok(())
}

fn create_compute_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let compute_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    compute_texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn create_render_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: shader,
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
            module: shader,
            entry_point: Some("main_fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    })
}

fn create_compute_bind_group(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    compute_texture_view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(compute_texture_view),
        }],
    })
}

fn create_render_bind_group(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    compute_texture_view: &wgpu::TextureView,
    compute_texture_sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(compute_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(compute_texture_sampler),
            },
        ],
    })
}

fn create_compute_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
) -> wgpu::ComputePipeline {
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: shader,
        entry_point: Some("main_cs"),
        compilation_options: Default::default(),
        cache: None,
    })
}
