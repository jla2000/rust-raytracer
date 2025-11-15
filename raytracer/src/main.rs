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
        .request_device(&wgpu::DeviceDescriptor::default())
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

    let window = &window;
    event_loop
        .run(move |event, active_loop| {
            if let winit::event::Event::WindowEvent { event, .. } = event {
                match event {
                    winit::event::WindowEvent::CloseRequested => active_loop.exit(),
                    winit::event::WindowEvent::RedrawRequested => {
                        match render(&surface, &device, &queue, &render_pipeline) {
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
) -> Result<(), wgpu::SurfaceError> {
    let output = surface.get_current_texture().unwrap();
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
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

    queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
}

fn main() {
    env_logger::init();
    pollster::block_on(run());
}
