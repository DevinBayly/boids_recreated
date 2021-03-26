use std::{borrow::Cow, mem};
use wgpu::util::DeviceExt;

use image::{DynamicImage,GenericImageView};

use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

const NUM_PARTICLES: u32 = 1500;

// number of particles per workgroup, oh yea work groups are fama

const PARTICLES_PER_GROUP: u32 = 64;

// this is like our state struct
struct State {
    swap_chain: wgpu::SwapChain,
    sc_desc:wgpu::SwapChainDescriptor,
    device: wgpu::Device,
    queue:wgpu::Queue,
    size: winit::dpi::PhysicalSize<u32>,
    surface: wgpu::Surface,
    particle_bind_groups: Vec<wgpu::BindGroup>,
    particle_buffers: Vec<wgpu::Buffer>,
    vertices_buffer: wgpu::Buffer,
    // pipelines
    render_pipeline: wgpu::RenderPipeline,
    compute_pipeline: wgpu::ComputePipeline,
    // specific to how we divide up the data going into the gpu to be worked on
    work_group_count: u32,
    frame_num: usize,
    texture_bind_group: wgpu::BindGroup,
}

impl State {
    async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        // get an instance
        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
        // get the screen
        let surface = unsafe { instance.create_surface(window) };
        // make the adapter from the instance
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("device and queue"),
                    features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES, // needed to make it so that the texture can be both read write
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap(); // second part is the trace path, dunno what this is though

        // make swap chain from device
        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            format: adapter.get_swap_chain_preferred_format(&surface),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        // here's the part of the code for the boids setup
        let mut flags = wgpu::ShaderFlags::VALIDATION;
        match adapter.get_info().backend {
            wgpu::Backend::Vulkan => {
                println!("yes vulkan fanboy time");
                flags |= wgpu::ShaderFlags::EXPERIMENTAL_TRANSLATION;
            }
            _ => {}
        }

        // get shader source
        let vs_src = include_str!("shader.vert");
        let fs_src = include_str!("shader.frag");
        let cs_src = include_str!("shader.comp");


        // compile
        let mut compiler = shaderc::Compiler::new().unwrap();
        let vs_spirv= compiler.compile_into_spirv(vs_src, shaderc::ShaderKind::Vertex,"shader.vert","main",None).unwrap();
        let fs_spirv = compiler.compile_into_spirv(fs_src, shaderc::ShaderKind::Fragment,"shader.frag","main",None).unwrap();
        let cs_spirv = compiler.compile_into_spirv(cs_src, shaderc::ShaderKind::Compute,"shader.comp","main",None).unwrap();

        // get data to load into module
        let vs_data = wgpu::util::make_spirv(vs_spirv.as_binary_u8());
        let fs_data = wgpu::util::make_spirv(fs_spirv.as_binary_u8());
        let cs_data = wgpu::util::make_spirv(cs_spirv.as_binary_u8());

        // make into module
        let vs_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor{
            label:Some("vert shader"),
            source: vs_data,
            flags: wgpu::ShaderFlags::default()
        });
        let fs_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor{
            label:Some("frag shader"),
            source: fs_data,
            flags: wgpu::ShaderFlags::default()
        });
        let cs_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor{
            label:Some("comp shader"),
            source: cs_data,
            flags: wgpu::ShaderFlags::default()
        });


        // texture with storage capability
        let og_image= image::DynamicImage::new_rgba8(256,256);
        let image_data= og_image.as_rgba8().unwrap();
        let dimensions = og_image.dimensions();
        // experiment with replacing the texture with raw data not using image crate
        // make a vector to hold our data 
        let raw_buffer_approach = [0u8;4*4*256*256];// will be used as the raw f32 texture, the first 4 is the number of bytes in a single color value, since we have 4 of those we have another 4 and then the image dimensions
        let texture_size = wgpu::Extent3d {
            width: 256,
            height:256,
            depth:1,
        };
        let texture  = device.create_texture( & wgpu::TextureDescriptor{
            size:texture_size,
            mip_level_count:1,
            sample_count:1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST | wgpu::TextureUsage::STORAGE,
            label:Some("writable texture")
        });

        queue.write_texture(
            wgpu::TextureCopyView{
                texture: &texture,
                mip_level:0,
                origin: wgpu::Origin3d::ZERO,
            },
            &raw_buffer_approach,
            wgpu::TextureDataLayout {
                offset:0,
                bytes_per_row:16*256,
                rows_per_image:256
            },
            texture_size
        );
        // make a view and a sampler 
        let texture_view = texture.create_view(& wgpu::TextureViewDescriptor::default());
        let texture_bind_group_layout = device.create_bind_group_layout(
            & wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry{
                        binding:0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty:wgpu::BindingType::StorageTexture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            format: wgpu::TextureFormat::Rgba32Float,
                            access: wgpu::StorageTextureAccess::ReadWrite
                        },
                        count:None
                    }
                ],
                label:Some("texture bind layout")
            }
        );
        // do the actual bind_group creation now that you have the layout
        let texture_bind_group = device.create_bind_group( & wgpu::BindGroupDescriptor{
            label:Some("texture bind group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view)
                },
            ]
        });


        // have to make a sampler too
        // texture bind group



        // use module in pipeline definitions


        let sim_param_data = [
            0.04f32, // deltaT
            0.1,     // rule1Distance
            0.025,   // rule2Distance
            0.025,   // rule3Distance
            0.02,    // rule1Scale
            0.05,    // rule2Scale
            0.005,   // rule3Scale
        ]
        .to_vec();

        let sim_param_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sim param buffer"),
            contents: bytemuck::cast_slice(&sim_param_data),
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });

        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                (sim_param_data.len() * mem::size_of::<f32>()) as _,
                            ),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new((NUM_PARTICLES * 16) as _),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new((NUM_PARTICLES * 16) as _),
                        },
                        count: None,
                    },
                ],
                label: Some("compute bind group"),
            });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("compute pipeline layout"),
                bind_group_layouts: &[&compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        // make a renderpipeline layout

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render pipeline layout"),
                bind_group_layouts: &[&texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vs_module,
                entry_point: "main",
                buffers: &[
                    // this is where desc was
                    // this is the first buffer holding data for position and velocity as vec2s stored one after another
                    wgpu::VertexBufferLayout {
                        array_stride: 4 * 4, // aha! because the triangle shape has 4 points in it!
                        step_mode: wgpu::InputStepMode::Instance, // the previous code used vertex info here, docs say this has to do with the way data gets advanced
                        attributes: &[
                            wgpu::VertexAttribute {
                                shader_location: 0,
                                offset: 0,
                                format: wgpu::VertexFormat::Float2, // basically a 2 vec still but with numeric type specified
                            },
                            wgpu::VertexAttribute {
                                shader_location: 1,
                                offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                                format: wgpu::VertexFormat::Float2,
                            },
                        ],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: 2 * 4,
                        step_mode: wgpu::InputStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            shader_location: 2,
                            offset: 0,
                            format: wgpu::VertexFormat::Float2,
                        }],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fs_module,
                entry_point: "main",
                targets: &[wgpu::ColorTargetState {
                    format: sc_desc.format,
                    // how the colors combine
                    alpha_blend: wgpu::BlendState::REPLACE,
                    color_blend: wgpu::BlendState::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
        });
        // now create the compute pipeline

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("compute pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &cs_module,
            entry_point: "main",
        });

        // making the buffers
        // buffer for the shape of our output, the arrow shape made of 2 triangles, so 6 vertices total
        // actually it makes more sense that this is the xy points for vertices of a single triangle
        let side_length = 0.005f32;
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
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
        });

        // make a buffer for the particles

        let mut initial_particle_data = vec![0.0f32; (4 * NUM_PARTICLES) as usize]; // *4 is because we have 2 pos and 2 velocity pieces for each particle
                                                                                    // go through and assign data
        for particle_instance_chunk in initial_particle_data.chunks_mut(4) {
            particle_instance_chunk[0] = 2.0 * (rand::random::<f32>() - 0.5); // the idea is to end up with position going -1 to 1 so subtract .5 and mult by 2
            particle_instance_chunk[1] = 2.0 * (rand::random::<f32>() - 0.5);
            particle_instance_chunk[2] = 2.0 * (rand::random::<f32>() - 0.5) * 0.1;
            particle_instance_chunk[3] = 2.0 * (rand::random::<f32>() - 0.5) * 0.1;
        }
        // now create the buffers
        // !!! this is how to have frames giving their results for going forward, make multiple buffers and alternate between src and dest
        let mut particle_buffers: Vec<wgpu::Buffer> = Vec::new();
        // interesting, have to learn about the STORAGE usage also
        for i in 0..2 {
            let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("one particle buffer {}", i)),
                contents: bytemuck::cast_slice(&initial_particle_data),
                usage: wgpu::BufferUsage::VERTEX
                    | wgpu::BufferUsage::STORAGE
                    | wgpu::BufferUsage::COPY_DST,
            });
            particle_buffers.push(buf);
        }
        let mut particle_bind_groups: Vec<wgpu::BindGroup> = Vec::new();
        // so clever! the first bind group gets the binding 1 to buffer at 0 and binding 2 and buffer 1
        // then the second group gets binding 1 at buffer 1, with binding 2 at buffer 0
        for i in 0..2 {
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        // used textureview and sampler before which have actual binding functions, but now we just have buffers so its different
                        resource: sim_param_buffer.as_entire_binding(),
                    },
                    // now the storage buffers for the simulation
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: particle_buffers[i].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: particle_buffers[(i + 1) % 2].as_entire_binding(),
                    },
                ],
                label: Some("bind groups"),
            });
            particle_bind_groups.push(group);
        }
        // calculate workgroups
        // will get a number used to break the total number of particles into groups
        let work_group_count =
            ((NUM_PARTICLES as f32) / (PARTICLES_PER_GROUP as f32)).ceil() as u32;

        State {
            swap_chain,
            sc_desc,
            device,
            queue,
            size,
            surface,
            work_group_count,
            frame_num: 0,
            // pipelines
            compute_pipeline,
            render_pipeline,
            // buffers
            particle_buffers,
            vertices_buffer,
            // groups
            particle_bind_groups,
            texture_bind_group
        }
    }
    fn render(&mut self) -> Result<(), wgpu::SwapChainError> {
        let frame = self.swap_chain.get_current_frame()?.output;

        // make encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });


        // wait until compute pass is complete before running the render pass
        encoder.push_debug_group("computing boid movement");
        {
            //println!("doing compute pass");
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label:None
            });
            // set pipeline and bindgroups then dispatch setting size
            cpass.set_pipeline(&self.compute_pipeline);
            cpass.set_bind_group(0, &self.particle_bind_groups[self.frame_num%2],&[]);// ok so this is how we get the right buffer into the compute pass, we keep a count of the frame we are on and select one or the other
            //?? but how  do we know to alternate between using one as source and other as dest? is this expressed in the wgsl?
            // this is a mixture of using 

            cpass.dispatch(self.work_group_count,1,1); // one dimensional compute shader,
        }
        encoder.pop_debug_group();


        // now do the render pass
        encoder.push_debug_group("doing draw pass");
        {
            // make the render pass
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view, // for writing to textures maybe this is the way to go?
                    resolve_target: None,
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
                depth_stencil_attachment:None
            });


            // set the pipeline
            render_pass.set_pipeline(& self.render_pipeline);

            // no bindgroups for the render pipeline this time
            // set the buffers
            render_pass.set_vertex_buffer(0, self.particle_buffers[(self.frame_num + 1)%2].slice(..));

            render_pass.set_vertex_buffer(1, self.vertices_buffer.slice(..));
            // set the texture bind group so something happens with it
            render_pass.set_bind_group(0,&self.texture_bind_group,&[]);


            // make draw call

            render_pass.draw(0..6,0..NUM_PARTICLES);
        }


        encoder.pop_debug_group();
        //?? I guess I'm still a bit lost about how one frame affects another specifying the bind group automatically 
        self.frame_num +=1;

        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.size = new_size;
        self.sc_desc.width = new_size.width;
        self.sc_desc.height = new_size.height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        false
    }
    fn update(&mut self) {}
}


fn main() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    use futures::executor::block_on;

    // Since main can't be async, we're going to need to block
    let mut state = block_on(State::new(&window));

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if !state.input(event) {
                    match event {
                        WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                        WindowEvent::KeyboardInput { input, .. } => match input {
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            } => *control_flow = ControlFlow::Exit,
                            _ => {}
                        },
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            // new_inner_size is &mut so w have to dereference it twice
                            state.resize(**new_inner_size);
                        }
                        _ => {}
                    }
                }
            }
            Event::RedrawRequested(_) => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    // Recreate the swap_chain if lost
                    Err(wgpu::SwapChainError::Lost) => state.resize(state.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SwapChainError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                window.request_redraw();
            }
            _ => {}
        }
    });
}