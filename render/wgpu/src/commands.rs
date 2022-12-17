use crate::frame::Frame;
use crate::mesh::{DrawType, Mesh};
use crate::{as_texture, ColorAdjustments, MaskState};
use ruffle_render::backend::ShapeHandle;
use ruffle_render::bitmap::BitmapHandle;
use ruffle_render::commands::{CommandHandler, CommandList};
use ruffle_render::transform::Transform;
use swf::{BlendMode, Color};

pub struct CommandRenderer<'a, 'b> {
    frame: &'b mut Frame<'a>,
    meshes: &'a Vec<Mesh>,
    quad_vertices: wgpu::BufferSlice<'a>,
    quad_indices: wgpu::BufferSlice<'a>,
    num_masks: u32,
}

impl<'a, 'b> CommandRenderer<'a, 'b> {
    pub fn new(
        frame: &'b mut Frame<'a>,
        meshes: &'a Vec<Mesh>,
        quad_vertices: wgpu::BufferSlice<'a>,
        quad_indices: wgpu::BufferSlice<'a>,
    ) -> Self {
        Self {
            frame,
            meshes,
            quad_vertices,
            quad_indices,
            num_masks: 0,
        }
    }
}

impl<'a, 'b> CommandHandler<'a> for CommandRenderer<'a, 'b> {
    fn render_bitmap(&mut self, bitmap: &'a BitmapHandle, transform: &Transform, smoothing: bool) {
        let texture = as_texture(bitmap);

        self.frame.apply_transform(
            &(transform.matrix
                * ruffle_render::matrix::Matrix {
                    a: texture.width as f32,
                    d: texture.height as f32,
                    ..Default::default()
                }),
            ColorAdjustments::from(transform.color_transform),
        );
        let descriptors = self.frame.descriptors();
        let bind = texture.bind_group(
            smoothing,
            &descriptors.device,
            &descriptors.bind_layouts.bitmap,
            &descriptors.quad,
            bitmap.clone(),
            &descriptors.bitmap_samplers,
        );

        self.frame.prep_bitmap(&bind.bind_group);

        self.frame.draw(self.quad_vertices, self.quad_indices, 6);
    }

    fn render_shape(&mut self, shape: ShapeHandle, transform: &Transform) {
        self.frame.apply_transform(
            &transform.matrix,
            ColorAdjustments::from(transform.color_transform),
        );

        let mesh = &self.meshes[shape.0];
        let mask_state = self.frame.mask_state();
        for draw in &mesh.draws {
            let num_indices = if mask_state != MaskState::DrawMaskStencil
                && mask_state != MaskState::ClearMaskStencil
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
                    self.frame.prep_color();
                }
                DrawType::Gradient { bind_group, .. } => {
                    self.frame.prep_gradient(bind_group);
                }
                DrawType::Bitmap { binds, .. } => {
                    self.frame.prep_bitmap(&binds.bind_group);
                }
            }

            self.frame.draw(
                draw.vertex_buffer.slice(..),
                draw.index_buffer.slice(..),
                num_indices,
            );
        }
    }

    fn draw_rect(&mut self, color: Color, matrix: &ruffle_render::matrix::Matrix) {
        self.frame.apply_transform(
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

        self.frame.prep_color();
        self.frame.draw(self.quad_vertices, self.quad_indices, 6);
    }

    fn push_mask(&mut self) {
        debug_assert!(
            self.frame.mask_state() == MaskState::NoMask
                || self.frame.mask_state() == MaskState::DrawMaskedContent
        );
        self.num_masks += 1;
        self.frame.set_mask_state(MaskState::DrawMaskStencil);
        self.frame.set_stencil(self.num_masks - 1);
    }

    fn activate_mask(&mut self) {
        debug_assert!(self.num_masks > 0 && self.frame.mask_state() == MaskState::DrawMaskStencil);
        self.frame.set_mask_state(MaskState::DrawMaskedContent);
        self.frame.set_stencil(self.num_masks);
    }

    fn deactivate_mask(&mut self) {
        debug_assert!(
            self.num_masks > 0 && self.frame.mask_state() == MaskState::DrawMaskedContent
        );
        self.frame.set_mask_state(MaskState::ClearMaskStencil);
        self.frame.set_stencil(self.num_masks);
    }

    fn pop_mask(&mut self) {
        debug_assert!(self.num_masks > 0 && self.frame.mask_state() == MaskState::ClearMaskStencil);
        self.num_masks -= 1;
        self.frame.set_stencil(self.num_masks);
        if self.num_masks == 0 {
            self.frame.set_mask_state(MaskState::NoMask);
        } else {
            self.frame.set_mask_state(MaskState::DrawMaskedContent);
        };
    }

    fn blend(&mut self, commands: &'a CommandList, _blend: BlendMode) {
        commands.execute(self);
    }
}
