use crate::mesh::{DrawType, Mesh};
use crate::surface::{BlendBuffer, DepthBuffer, FrameBuffer, ResolveBuffer, TextureBuffers};
use crate::{
    as_texture, ColorAdjustments, Descriptors, Globals, MaskState, Pipelines, TextureTransforms,
    Transforms, UniformBuffer,
};
use ruffle_render::backend::ShapeHandle;
use ruffle_render::bitmap::BitmapHandle;
use ruffle_render::commands::{Command, CommandList};
use ruffle_render::transform::Transform;
use swf::{BlendMode, Color};

pub struct CommandTarget<'pass> {
    frame_buffer: &'pass FrameBuffer,
    blend_buffer: &'pass BlendBuffer,
    resolve_buffer: Option<&'pass ResolveBuffer>,
    depth: &'pass DepthBuffer,
}

impl<'pass> CommandTarget<'pass> {
    pub fn new(
        frame_buffer: &'pass FrameBuffer,
        blend_buffer: &'pass BlendBuffer,
        resolve_buffer: Option<&'pass ResolveBuffer>,
        depth: &'pass DepthBuffer,
    ) -> Self {
        Self {
            frame_buffer,
            blend_buffer,
            resolve_buffer,
            depth,
        }
    }

    pub fn color_attachments(&self, clear: bool) -> Option<wgpu::RenderPassColorAttachment<'pass>> {
        Some(wgpu::RenderPassColorAttachment {
            view: &self.frame_buffer.view(),
            resolve_target: self.resolve_buffer.as_ref().map(|b| b.view()),
            ops: wgpu::Operations {
                load: if clear {
                    wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                } else {
                    wgpu::LoadOp::Load
                },
                store: true,
            },
        })
    }

    pub fn depth_attachment(
        &self,
        clear: bool,
    ) -> Option<wgpu::RenderPassDepthStencilAttachment<'pass>> {
        Some(wgpu::RenderPassDepthStencilAttachment {
            view: self.depth.view(),
            depth_ops: Some(wgpu::Operations {
                load: if clear {
                    wgpu::LoadOp::Clear(0.0)
                } else {
                    wgpu::LoadOp::Load
                },
                store: true,
            }),
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: true,
            }),
        })
    }

    pub fn update_blend_buffer(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTextureBase {
                texture: self
                    .resolve_buffer
                    .as_ref()
                    .map(|b| b.texture())
                    .unwrap_or_else(|| self.frame_buffer.texture()),
                mip_level: 0,
                origin: Default::default(),
                aspect: Default::default(),
            },
            wgpu::ImageCopyTextureBase {
                texture: self.blend_buffer.texture(),
                mip_level: 0,
                origin: Default::default(),
                aspect: Default::default(),
            },
            self.frame_buffer.size(),
        );
    }

    pub fn color_view(&self) -> &wgpu::TextureView {
        self.resolve_buffer
            .as_ref()
            .map(|b| b.view())
            .unwrap_or_else(|| self.frame_buffer.view())
    }
}

pub struct CommandRenderer<'pass, 'frame: 'pass, 'global: 'frame> {
    pipelines: &'frame Pipelines,
    meshes: &'global Vec<Mesh>,
    descriptors: &'global Descriptors,
    num_masks: u32,
    mask_state: MaskState,
    render_pass: wgpu::RenderPass<'pass>,
    uniform_buffers: &'frame mut UniformBuffer<'global, Transforms>,
    uniform_encoder: &'frame mut wgpu::CommandEncoder,
}

impl<'pass, 'frame: 'pass, 'global: 'frame> CommandRenderer<'pass, 'frame, 'global> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pipelines: &'frame Pipelines,
        meshes: &'global Vec<Mesh>,
        descriptors: &'global Descriptors,
        uniform_buffers: &'frame mut UniformBuffer<'global, Transforms>,
        uniform_encoder: &'frame mut wgpu::CommandEncoder,
        render_pass: wgpu::RenderPass<'pass>,
        num_masks: u32,
        mask_state: MaskState,
    ) -> Self {
        Self {
            pipelines,
            meshes,
            num_masks,
            mask_state,
            render_pass,
            descriptors,
            uniform_buffers,
            uniform_encoder,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        globals: &'frame Globals,
        pipelines: &'frame Pipelines,
        texture_buffers: &'frame mut TextureBuffers,
        target: &'pass CommandTarget<'pass>,
        meshes: &'global Vec<Mesh>,
        descriptors: &'global Descriptors,
        uniform_buffers: &'frame mut UniformBuffer<'global, Transforms>,
        uniform_encoder: &'frame mut wgpu::CommandEncoder,
        commands: CommandList,
        output: &mut Vec<wgpu::CommandBuffer>,
        nearest_layer: &'pass CommandTarget<'pass>,
        clear_depth: bool,
    ) {
        let label = create_debug_label!("Draw encoder");
        let mut draw_encoder =
            descriptors
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: label.as_deref(),
                });
        let mut first = true;
        let mut num_masks = 0;
        let mut mask_state = MaskState::NoMask;

        for chunk in chunk_blends(commands.0) {
            match chunk {
                Chunk::Draw(chunk) => {
                    let mut render_pass =
                        draw_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[target.color_attachments(first)],
                            depth_stencil_attachment: target.depth_attachment(first && clear_depth),
                        });
                    render_pass.set_bind_group(0, globals.bind_group(), &[]);
                    let mut renderer = CommandRenderer::new(
                        &pipelines,
                        &meshes,
                        &descriptors,
                        uniform_buffers,
                        uniform_encoder,
                        render_pass,
                        num_masks,
                        mask_state,
                    );

                    for command in &chunk {
                        match renderer.mask_state {
                            MaskState::NoMask => {}
                            MaskState::DrawMaskStencil => {
                                renderer
                                    .render_pass
                                    .set_stencil_reference(renderer.num_masks - 1);
                            }
                            MaskState::DrawMaskedContent => {
                                renderer
                                    .render_pass
                                    .set_stencil_reference(renderer.num_masks);
                            }
                            MaskState::ClearMaskStencil => {
                                renderer
                                    .render_pass
                                    .set_stencil_reference(renderer.num_masks);
                            }
                        }

                        match command {
                            Command::RenderBitmap {
                                bitmap,
                                transform,
                                smoothing,
                            } => renderer.render_bitmap(bitmap, &transform, *smoothing),
                            Command::RenderShape { shape, transform } => {
                                renderer.render_shape(*shape, &transform)
                            }
                            Command::DrawRect { color, matrix } => {
                                renderer.draw_rect(color.clone(), &matrix)
                            }
                            Command::PushMask => renderer.push_mask(),
                            Command::ActivateMask => renderer.activate_mask(),
                            Command::DeactivateMask => renderer.deactivate_mask(),
                            Command::PopMask => renderer.pop_mask(),
                            Command::Blend(_, _) => {
                                unreachable!("Command::Blend is separated out")
                            }
                        }
                    }

                    num_masks = renderer.num_masks;
                    mask_state = renderer.mask_state;
                }
                Chunk::Blend(commands, blend_mode) => {
                    target.update_blend_buffer(&mut draw_encoder);

                    let frame_buffer = texture_buffers.take_frame_buffer(&descriptors);
                    let resolve_buffer = texture_buffers.take_resolve_buffer(&descriptors);
                    let blend_buffer = texture_buffers.take_blend_buffer(&descriptors);

                    let child = CommandTarget::new(
                        &frame_buffer,
                        &blend_buffer,
                        resolve_buffer.as_ref(),
                        &target.depth,
                    );

                    CommandRenderer::execute(
                        &globals,
                        &pipelines,
                        texture_buffers,
                        &child,
                        &meshes,
                        &descriptors,
                        uniform_buffers,
                        uniform_encoder,
                        commands,
                        output,
                        if blend_mode == BlendMode::Layer {
                            &child
                        } else {
                            nearest_layer
                        },
                        false,
                    );

                    let bitmap_bind_group =
                        descriptors
                            .device
                            .create_bind_group(&wgpu::BindGroupDescriptor {
                                label: None,
                                layout: &descriptors.bind_layouts.bitmap,
                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: wgpu::BindingResource::Buffer(
                                            wgpu::BufferBinding {
                                                buffer: &descriptors.quad.texture_transforms,
                                                offset: 0,
                                                size: wgpu::BufferSize::new(std::mem::size_of::<
                                                    TextureTransforms,
                                                >(
                                                )
                                                    as u64),
                                            },
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 1,
                                        resource: wgpu::BindingResource::TextureView(
                                            child.color_view(),
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 2,
                                        resource: wgpu::BindingResource::Sampler(
                                            &descriptors.bitmap_samplers.get_sampler(false, false),
                                        ),
                                    },
                                ],
                            });
                    let mut render_pass =
                        draw_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[target.color_attachments(first)],
                            depth_stencil_attachment: child.depth_attachment(false),
                        });
                    render_pass.set_bind_group(0, globals.bind_group(), &[]);

                    match mask_state {
                        MaskState::NoMask => {}
                        MaskState::DrawMaskStencil => {
                            render_pass.set_stencil_reference(num_masks - 1);
                        }
                        MaskState::DrawMaskedContent => {
                            render_pass.set_stencil_reference(num_masks);
                        }
                        MaskState::ClearMaskStencil => {
                            render_pass.set_stencil_reference(num_masks);
                        }
                    }

                    render_pass.set_pipeline(pipelines.bitmap.pipeline_for(mask_state));

                    render_pass.set_bind_group(1, texture_buffers.whole_frame_bind_group(), &[0]);
                    render_pass.set_bind_group(2, &bitmap_bind_group, &[]);

                    render_pass.set_vertex_buffer(0, descriptors.quad.vertices.slice(..));
                    render_pass.set_index_buffer(
                        descriptors.quad.indices.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );

                    render_pass.draw_indexed(0..6, 0, 0..1);
                    drop(render_pass);
                }
            }
            first = false;
        }

        output.push(draw_encoder.finish());
    }

    pub fn prep_color(&mut self) {
        self.render_pass
            .set_pipeline(self.pipelines.color.pipeline_for(self.mask_state));
    }

    pub fn prep_gradient(&mut self, bind_group: &'pass wgpu::BindGroup) {
        self.render_pass
            .set_pipeline(self.pipelines.gradient.pipeline_for(self.mask_state));

        self.render_pass.set_bind_group(2, bind_group, &[]);
    }

    pub fn prep_bitmap(&mut self, bind_group: &'pass wgpu::BindGroup) {
        self.render_pass
            .set_pipeline(self.pipelines.bitmap.pipeline_for(self.mask_state));

        self.render_pass.set_bind_group(2, bind_group, &[]);
    }

    pub fn draw(
        &mut self,
        vertices: wgpu::BufferSlice<'pass>,
        indices: wgpu::BufferSlice<'pass>,
        num_indices: u32,
    ) {
        self.render_pass.set_vertex_buffer(0, vertices);
        self.render_pass
            .set_index_buffer(indices, wgpu::IndexFormat::Uint32);

        self.render_pass.draw_indexed(0..num_indices, 0, 0..1);
    }

    pub fn apply_transform(
        &mut self,
        matrix: &ruffle_render::matrix::Matrix,
        color_adjustments: ColorAdjustments,
    ) {
        let world_matrix = [
            [matrix.a, matrix.b, 0.0, 0.0],
            [matrix.c, matrix.d, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [
                matrix.tx.to_pixels() as f32,
                matrix.ty.to_pixels() as f32,
                0.0,
                1.0,
            ],
        ];

        self.uniform_buffers.write_uniforms(
            &self.descriptors.device,
            &self.descriptors.bind_layouts.transforms,
            &mut self.uniform_encoder,
            &mut self.render_pass,
            1,
            &Transforms {
                world_matrix,
                color_adjustments,
            },
        );
    }

    fn render_bitmap(
        &mut self,
        bitmap: &'frame BitmapHandle,
        transform: &Transform,
        smoothing: bool,
    ) {
        let texture = as_texture(bitmap);

        self.apply_transform(
            &(transform.matrix
                * ruffle_render::matrix::Matrix {
                    a: texture.width as f32,
                    d: texture.height as f32,
                    ..Default::default()
                }),
            ColorAdjustments::from(transform.color_transform),
        );
        let descriptors = self.descriptors;
        let bind = texture.bind_group(
            smoothing,
            &descriptors.device,
            &descriptors.bind_layouts.bitmap,
            &descriptors.quad,
            bitmap.clone(),
            &descriptors.bitmap_samplers,
        );

        self.prep_bitmap(&bind.bind_group);

        self.draw(
            self.descriptors.quad.vertices.slice(..),
            self.descriptors.quad.indices.slice(..),
            6,
        );
    }

    fn render_shape(&mut self, shape: ShapeHandle, transform: &Transform) {
        self.apply_transform(
            &transform.matrix,
            ColorAdjustments::from(transform.color_transform),
        );

        let mesh = &self.meshes[shape.0];
        for draw in &mesh.draws {
            let num_indices = if self.mask_state != MaskState::DrawMaskStencil
                && self.mask_state != MaskState::ClearMaskStencil
            {
                draw.num_indices
            } else {
                // Omit strokes when drawing a mask stencil.
                draw.num_mask_indices
            };
            if num_indices == 0 {
                continue;
            }

            match &draw.draw_type {
                DrawType::Color => {
                    self.prep_color();
                }
                DrawType::Gradient { bind_group, .. } => {
                    self.prep_gradient(bind_group);
                }
                DrawType::Bitmap { binds, .. } => {
                    self.prep_bitmap(&binds.bind_group);
                }
            }

            self.draw(
                draw.vertex_buffer.slice(..),
                draw.index_buffer.slice(..),
                num_indices,
            );
        }
    }

    fn draw_rect(&mut self, color: Color, matrix: &ruffle_render::matrix::Matrix) {
        self.apply_transform(
            &matrix,
            ColorAdjustments {
                mult_color: [
                    f32::from(color.r) / 255.0,
                    f32::from(color.g) / 255.0,
                    f32::from(color.b) / 255.0,
                    f32::from(color.a) / 255.0,
                ],
                add_color: [0.0, 0.0, 0.0, 0.0],
            },
        );

        self.prep_color();
        self.draw(
            self.descriptors.quad.vertices.slice(..),
            self.descriptors.quad.indices.slice(..),
            6,
        );
    }

    fn push_mask(&mut self) {
        debug_assert!(
            self.mask_state == MaskState::NoMask || self.mask_state == MaskState::DrawMaskedContent
        );
        self.num_masks += 1;
        self.mask_state = MaskState::DrawMaskStencil;
        self.render_pass.set_stencil_reference(self.num_masks - 1);
    }

    fn activate_mask(&mut self) {
        debug_assert!(self.num_masks > 0 && self.mask_state == MaskState::DrawMaskStencil);
        self.mask_state = MaskState::DrawMaskedContent;
        self.render_pass.set_stencil_reference(self.num_masks);
    }

    fn deactivate_mask(&mut self) {
        debug_assert!(self.num_masks > 0 && self.mask_state == MaskState::DrawMaskedContent);
        self.mask_state = MaskState::ClearMaskStencil;
        self.render_pass.set_stencil_reference(self.num_masks);
    }

    fn pop_mask(&mut self) {
        debug_assert!(self.num_masks > 0 && self.mask_state == MaskState::ClearMaskStencil);
        self.num_masks -= 1;
        self.render_pass.set_stencil_reference(self.num_masks);
        if self.num_masks == 0 {
            self.mask_state = MaskState::NoMask;
        } else {
            self.mask_state = MaskState::DrawMaskedContent;
        };
    }
}

#[derive(Debug)]
pub enum Chunk {
    Draw(Vec<Command>),
    Blend(CommandList, BlendMode),
}

/// Chunk the commands such that every Blend is separated out
fn chunk_blends(commands: Vec<Command>) -> Vec<Chunk> {
    let mut result = vec![];
    let mut current = vec![];

    for command in commands {
        match command {
            Command::Blend(commands, blend_mode) => {
                if !current.is_empty() {
                    result.push(Chunk::Draw(std::mem::take(&mut current)));
                }
                result.push(Chunk::Blend(commands, blend_mode));
            }
            _ => {
                current.push(command);
            }
        }
    }

    if !current.is_empty() {
        result.push(Chunk::Draw(current));
    }

    result
}
