// starting over from scratch because it's been a while since I wrote anything like this
use wgpu::util::DeviceExt;
use std::fs::write;
use std::fs::read;
mod naga_convert;
use crate::naga_convert::conversion_tools::convert_src;

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
    let texture_size = 3840_u32/2;
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
 // making the vertex buffers
        // buffer for the shape of our output, the arrow shape made of 2 triangles, so 6 vertices total
        // actually it makes more sense that this is the xy points for vertices of a single triangle
        let side_length = 0.0001f32;
        let vertex_buffer_data = [-side_length,-side_length,
        side_length,-side_length,
        side_length,side_length,
        // second triangle now
        side_length,side_length,
        -side_length,side_length,
        -side_length,-side_length]; // ? so is this x and Y values totgether? or are we doing something in the shader code to take this and produce x y values?
                                                                             // ? mystery about how the other half of the shape gets drawn , but maybe that will become clear in the render
                                                                             // create actual buffer from the data
        let vertices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex buffer"),
            contents: bytemuck::cast_slice(&vertex_buffer_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

// here's where we will read in the hdf5 data
    let mut f = hdf5::File::open("./positions_chunk6.hdf5").unwrap();
    let mut dataset = f.dataset("snapshot_069").unwrap();
    let data = dataset.read_2d::<f32>().unwrap();
    let pbuffer = data.as_slice().unwrap();

    let points_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
        label:Some("hdf5 points"),
        contents:bytemuck::cast_slice(&pbuffer),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST
    });

    

    // bring in and compile the shaders
    // then make descriptor,layout, and eventually a pipeline
    // let vs_source = include_str!("shader.vert");
    // let fs_source = include_str!("shader.frag");
    // let mut compiler = shaderc::Compiler::new().unwrap();
    // let vs_spirv = compiler
    //     .compile_into_spirv(
    //         vs_source,
    //         shaderc::ShaderKind::Vertex,
    //         "shader.vert",
    //         "main",
    //         None,
    //     )
    //     .unwrap();
    // let fs_spirv = compiler
    //     .compile_into_spirv(
    //         fs_source,
    //         shaderc::ShaderKind::Fragment,
    //         "shader.frag",
    //         "main",
    //         None,
    //     )
    //     .unwrap();
    // let vbin = read("./vert_binary").unwrap();
    // let fbin = read("./frag_binary").unwrap();
    let vs_data = convert_src("./src/shader.vert");
    let fs_data = convert_src("./src/shader.frag");
    
    let vs_data = wgpu::ShaderSource::SpirV(vs_data.into());
    let fs_data = wgpu::ShaderSource::SpirV(fs_data.into());
    // let vs_data = wgpu::util::make_spirv(&vbin);
    // let fs_data = wgpu::util::make_spirv(&fbin);
    //write out the spirv to use on HPC
    // write("vert_binary",&vs_spirv.as_binary_u8());
    // write("frag_binary",&fs_spirv.as_binary_u8());

    // the astro data needs to be brought in like the particle buffes from before
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
            buffers: &[
                wgpu::VertexBufferLayout{
                    array_stride:2*4, // a f32 is 4bytes and x,y 2d is 2
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes:&[wgpu::VertexAttribute{
                        shader_location:0,
                        offset:0,
                        format:wgpu::VertexFormat::Float32x2 //basically a vec2
                    }
                    ]
                },
                wgpu::VertexBufferLayout{
                    array_stride:3*4, //three values per instance, xyz
                    step_mode:wgpu::VertexStepMode::Instance,
                    attributes : &[
                        wgpu::VertexAttribute{
                            shader_location:1,
                            offset:0,
                            format:wgpu::VertexFormat::Float32x3
                        }
                    ]
                }
            ],
            entry_point: "main",
        },
        fragment: Some(wgpu::FragmentState {
            module: &fs_module,
            // note no buffers, is this
            entry_point: "main",
            targets: &[wgpu::ColorTargetState {
                format: texture_description.format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
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
        render_pass.set_vertex_buffer(0, vertices_buffer.slice(..));
        render_pass.set_vertex_buffer(1, points_buffer.slice(..));
        println!("made it to the draw call");
        render_pass.draw(0..6, 0..(pbuffer.len()/3) as u32);
    }

    // this is the step where we are going to be putting the stuff from window in an image to bring back
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
    // and now we take the buffer and make an image out of it and save it?
    queue.submit(Some(encoder.finish()));
    {
        let buffer_slice = buffer.slice(..);
        let mapping = buffer_slice.map_async(wgpu::MapMode::Read);
        //apparently if this step isn't done then the application freezes`
        device.poll(wgpu::Maintain::Wait);
        mapping.await.unwrap();

        let data = buffer_slice.get_mapped_range();
        use image::{ImageBuffer,Rgba};

        let buffer_im = ImageBuffer::<Rgba<u8>,_>::from_raw(texture_size,texture_size,data).unwrap();
        buffer_im.save("image.png").unwrap();

    }
    buffer.unmap();
}

fn main() {
    pollster::block_on(run());
}
