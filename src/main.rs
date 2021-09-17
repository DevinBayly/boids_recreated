// starting over from scratch because it's been a while since I wrote anything like this

use std::num::NonZeroU32;

async fn run() {
    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY); // default to what ever is the
                                                                 // now get the first adapter
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
        })
        .await
        .unwrap();
    // now go get a device and the command queue
    let (device, queue): (wgpu::Device, wgpu::Queue) = adapter
        .request_device(&Default::default(), None)
        .await
        .unwrap();
    // need to make texture
    let texture_size = 256_u32;
    // make a texture description, using struct
    let texture_description = wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width: texture_size,
            height: texture_size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2, // 2d texture after all
        format: wgpu::TextureFormat::Rgba8UnormSrgb, //
        // want the usage to be copy SRC meaning that we can copy from and also make sure it is "the output attachment of a renderpass"
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        label: None,
    };
    // device makes the texture
    let texture = device.create_texture(&texture_description);
    let texture_view = texture.create_view(&Default::default());
    // create the buffer where we are going to put the texture data before saving it
    let u32_size = std::mem::size_of::<u32>() as u32;
    let output_buffer_size = (u32_size * texture_size * texture_size) as wgpu::BufferAddress;
    let output_buffer_desc = wgpu::BufferDescriptor {
        // size and usage is common to descriptors so far
        size: output_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, // this means we need to be able to copy to it as well as read from it by cpu
        label: None,
        mapped_at_creation: false,
    };
    // device makes buffer just like making the texture
    let buffer = device.create_buffer(&output_buffer_desc);
    // bring in and compile the shaders
    // then make descriptor,layout, and eventually a pipeline
    let vs_source = include_str!("shader.vert");
    let fs_source = include_str!("shader.frag");
    let mut compiler = shaderc::Compiler::new().unwrap();
    let vs_spirv = compiler
        .compile_into_spirv(
            vs_source,
            shaderc::ShaderKind::Vertex,
            "shader.vert",
            "main",
            None,
        )
        .unwrap();
    let fs_spirv = compiler
        .compile_into_spirv(
            fs_source,
            shaderc::ShaderKind::Fragment,
            "shader.frag",
            "main",
            None,
        )
        .unwrap();

    let vs_data = wgpu::util::make_spirv(&vs_spirv.as_binary_u8());
    let fs_data = wgpu::util::make_spirv(&fs_spirv.as_binary_u8());

    //make the modules

    let vs_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: Some("vertex shader"),
        source: vs_data,
    });
    let fs_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: Some(" fragment shader"),
        source: fs_data,
    });
    // making the pipelines
    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        // no bindgroups
        label: Some("rp layout"),
        bind_group_layouts: &[], // this is where we would put a texture that we are binding or something
        push_constant_ranges: &[],
    });
    println!("made it this far");
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("rp"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &vs_module,
            buffers: &[],
            entry_point: "main",
        },
        fragment: Some(wgpu::FragmentState {
            module: &fs_module,
            // note no buffers, is this
            entry_point: "main",
            targets: &[wgpu::ColorTargetState {
                format: texture_description.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            }],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,         // not sure what this is
            front_face: wgpu::FrontFace::Ccw, // this is the winding for the vertex list to make the mesh face
            cull_mode: Some(wgpu::Face::Back),
            clamp_depth: false,
            conservative: false,
            polygon_mode: wgpu::PolygonMode::Fill,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            // most ojf this is unclear to me.
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
    });

    // make an encoder
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    // now we create the render pass

    {
        let render_pass_desc = wgpu::RenderPassDescriptor {
            label: Some("render pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &texture_view,
                resolve_target: None, // I think this is because we are rendering without a window target is None
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.5,
                        a: 1.0,
                    }),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        };
        let mut render_pass = encoder.begin_render_pass(&render_pass_desc);
        // now we bring the pipeline back in
        render_pass.set_pipeline(&render_pipeline);
        println!("made it to the draw call");
        render_pass.draw(0..3, 0..1);
    }


    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            aspect: wgpu::TextureAspect::All,
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(u32_size * texture_size),
                rows_per_image: NonZeroU32::new(texture_size),
            },
        },
        texture_description.size,
    );
}

fn main() {
    pollster::block_on(run());
}
