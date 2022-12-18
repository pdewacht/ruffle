use crate::layouts::BindLayouts;
use crate::pipelines::VERTEX_BUFFERS_DESCRIPTION;
use crate::shaders::Shaders;
use crate::{create_buffer_with_data, BitmapSamplers, Pipelines, TextureTransforms, Vertex};
use fnv::FnvHashMap;
use std::sync::{Arc, Mutex};
use swf::BlendMode;

const MAX_BLEND_MODES: usize = 13;

pub struct Descriptors {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub limits: wgpu::Limits,
    pub queue: wgpu::Queue,
    pub bitmap_samplers: BitmapSamplers,
    pub bind_layouts: BindLayouts,
    pub quad: Quad,
    blend_buffers: [wgpu::Buffer; MAX_BLEND_MODES],
    copy_pipeline: Mutex<FnvHashMap<wgpu::TextureFormat, Arc<wgpu::RenderPipeline>>>,
    copy_srgb_pipeline: Mutex<FnvHashMap<wgpu::TextureFormat, Arc<wgpu::RenderPipeline>>>,
    shaders: Shaders,
    pipelines: Mutex<FnvHashMap<(u32, wgpu::TextureFormat), Arc<Pipelines>>>,
}

impl Descriptors {
    pub fn new(adapter: wgpu::Adapter, device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let limits = device.limits();
        let bind_layouts = BindLayouts::new(&device);
        let bitmap_samplers = BitmapSamplers::new(&device);
        let shaders = Shaders::new(&device);
        let quad = Quad::new(&device);

        let blend_buffers = core::array::from_fn(|blend_id| {
            create_buffer_with_data(
                &device,
                bytemuck::cast_slice(&[blend_id, 0, 0, 0]),
                wgpu::BufferUsages::UNIFORM,
                create_debug_label!("Blend mode {:?}", blend_id),
            )
        });

        Self {
            adapter,
            device,
            limits,
            queue,
            bitmap_samplers,
            bind_layouts,
            quad,
            blend_buffers,
            copy_pipeline: Default::default(),
            copy_srgb_pipeline: Default::default(),
            shaders,
            pipelines: Default::default(),
        }
    }

    pub fn copy_srgb_pipeline(&self, format: wgpu::TextureFormat) -> Arc<wgpu::RenderPipeline> {
        let mut pipelines = self
            .copy_srgb_pipeline
            .lock()
            .expect("Pipelines should not be already locked");
        pipelines
            .entry(format)
            .or_insert_with(|| {
                let copy_texture_pipeline_layout =
                    &self
                        .device
                        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: create_debug_label!("Copy sRGB pipeline layout").as_deref(),
                            bind_group_layouts: &[
                                &self.bind_layouts.globals,
                                &self.bind_layouts.transforms,
                                &self.bind_layouts.bitmap,
                            ],
                            push_constant_ranges: &[],
                        });
                Arc::new(
                    self.device
                        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: create_debug_label!("Copy sRGB pipeline").as_deref(),
                            layout: Some(&copy_texture_pipeline_layout),
                            vertex: wgpu::VertexState {
                                module: &self.shaders.copy_srgb_shader,
                                entry_point: "main_vertex",
                                buffers: &VERTEX_BUFFERS_DESCRIPTION,
                            },
                            fragment: Some(wgpu::FragmentState {
                                module: &self.shaders.copy_srgb_shader,
                                entry_point: "main_fragment",
                                targets: &[Some(wgpu::ColorTargetState {
                                    format,
                                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                                    write_mask: Default::default(),
                                })],
                            }),
                            primitive: wgpu::PrimitiveState {
                                topology: wgpu::PrimitiveTopology::TriangleList,
                                strip_index_format: None,
                                front_face: wgpu::FrontFace::Ccw,
                                cull_mode: None,
                                polygon_mode: wgpu::PolygonMode::default(),
                                unclipped_depth: false,
                                conservative: false,
                            },
                            depth_stencil: None,
                            multisample: wgpu::MultisampleState {
                                count: 1,
                                mask: !0,
                                alpha_to_coverage_enabled: false,
                            },
                            multiview: None,
                        }),
                )
            })
            .clone()
    }

    pub fn copy_pipeline(&self, format: wgpu::TextureFormat) -> Arc<wgpu::RenderPipeline> {
        let mut pipelines = self
            .copy_pipeline
            .lock()
            .expect("Pipelines should not be already locked");
        pipelines
            .entry(format)
            .or_insert_with(|| {
                let copy_texture_pipeline_layout =
                    &self
                        .device
                        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: create_debug_label!("Copy pipeline layout").as_deref(),
                            bind_group_layouts: &[
                                &self.bind_layouts.globals,
                                &self.bind_layouts.transforms,
                                &self.bind_layouts.bitmap,
                            ],
                            push_constant_ranges: &[],
                        });
                Arc::new(
                    self.device
                        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: create_debug_label!("Copy pipeline").as_deref(),
                            layout: Some(&copy_texture_pipeline_layout),
                            vertex: wgpu::VertexState {
                                module: &self.shaders.copy_shader,
                                entry_point: "main_vertex",
                                buffers: &VERTEX_BUFFERS_DESCRIPTION,
                            },
                            fragment: Some(wgpu::FragmentState {
                                module: &self.shaders.copy_shader,
                                entry_point: "main_fragment",
                                targets: &[Some(wgpu::ColorTargetState {
                                    format,
                                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                                    write_mask: Default::default(),
                                })],
                            }),
                            primitive: wgpu::PrimitiveState {
                                topology: wgpu::PrimitiveTopology::TriangleList,
                                strip_index_format: None,
                                front_face: wgpu::FrontFace::Ccw,
                                cull_mode: None,
                                polygon_mode: wgpu::PolygonMode::default(),
                                unclipped_depth: false,
                                conservative: false,
                            },
                            depth_stencil: None,
                            multisample: wgpu::MultisampleState {
                                count: 1,
                                mask: !0,
                                alpha_to_coverage_enabled: false,
                            },
                            multiview: None,
                        }),
                )
            })
            .clone()
    }

    pub fn pipelines(&self, msaa_sample_count: u32, format: wgpu::TextureFormat) -> Arc<Pipelines> {
        let mut pipelines = self
            .pipelines
            .lock()
            .expect("Pipelines should not be already locked");
        pipelines
            .entry((msaa_sample_count, format))
            .or_insert_with(|| {
                Arc::new(Pipelines::new(
                    &self.device,
                    &self.shaders,
                    format,
                    msaa_sample_count,
                    &self.bind_layouts,
                ))
            })
            .clone()
    }

    pub fn blend_buffer(&self, blend_mode: BlendMode) -> &wgpu::Buffer {
        let index = match blend_mode {
            BlendMode::Normal => 0,
            BlendMode::Layer => 0,
            BlendMode::Multiply => 1,
            BlendMode::Screen => 2,
            BlendMode::Lighten => 3,
            BlendMode::Darken => 4,
            BlendMode::Difference => 5,
            BlendMode::Add => 6,
            BlendMode::Subtract => 7,
            BlendMode::Invert => 8,
            BlendMode::Alpha => 9,
            BlendMode::Erase => 10,
            BlendMode::Overlay => 11,
            BlendMode::HardLight => 12,
        };
        &self.blend_buffers[index]
    }
}

pub struct Quad {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub texture_transforms: wgpu::Buffer,
}

impl Quad {
    pub fn new(device: &wgpu::Device) -> Self {
        let vertices = [
            Vertex {
                position: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            Vertex {
                position: [1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            Vertex {
                position: [1.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            Vertex {
                position: [0.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
        ];
        let indices: [u32; 6] = [0, 1, 2, 0, 2, 3];

        let vbo = create_buffer_with_data(
            device,
            bytemuck::cast_slice(&vertices),
            wgpu::BufferUsages::VERTEX,
            create_debug_label!("Quad vbo"),
        );

        let ibo = create_buffer_with_data(
            device,
            bytemuck::cast_slice(&indices),
            wgpu::BufferUsages::INDEX,
            create_debug_label!("Quad ibo"),
        );

        let tex_transforms = create_buffer_with_data(
            device,
            bytemuck::cast_slice(&[TextureTransforms {
                u_matrix: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
            }]),
            wgpu::BufferUsages::UNIFORM,
            create_debug_label!("Quad tex transforms"),
        );

        Self {
            vertices: vbo,
            indices: ibo,
            texture_transforms: tex_transforms,
        }
    }
}
