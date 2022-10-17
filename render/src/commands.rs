use crate::backend::ShapeHandle;
use crate::bitmap::BitmapHandle;
use crate::matrix::Matrix;
use crate::transform::Transform;
use swf::{BlendMode, Color};

pub trait CommandHandler {
    fn render_bitmap(&mut self, bitmap: BitmapHandle, transform: &Transform, smoothing: bool);
    fn render_shape(&mut self, shape: ShapeHandle, transform: &Transform);
    fn draw_rect(&mut self, color: Color, matrix: &Matrix);
    fn push_mask(&mut self);
    fn activate_mask(&mut self);
    fn deactivate_mask(&mut self);
    fn pop_mask(&mut self);

    fn blend(&mut self, commands: CommandList, blend_mode: BlendMode);
}

#[derive(Debug, Default)]
pub struct CommandList(pub Vec<Command>);

impl CommandList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(self, handler: &mut impl CommandHandler) {
        for command in self.0 {
            match command {
                Command::RenderBitmap {
                    bitmap,
                    transform,
                    smoothing,
                } => handler.render_bitmap(bitmap, &transform, smoothing),
                Command::RenderShape { shape, transform } => {
                    handler.render_shape(shape, &transform)
                }
                Command::DrawRect { color, matrix } => handler.draw_rect(color, &matrix),
                Command::PushMask => handler.push_mask(),
                Command::ActivateMask => handler.activate_mask(),
                Command::DeactivateMask => handler.deactivate_mask(),
                Command::PopMask => handler.pop_mask(),
                Command::Blend(commands, blend_mode) => handler.blend(commands, blend_mode),
            }
        }
    }
}

impl CommandHandler for CommandList {
    fn render_bitmap(&mut self, bitmap: BitmapHandle, transform: &Transform, smoothing: bool) {
        self.0.push(Command::RenderBitmap {
            bitmap,
            transform: transform.clone(),
            smoothing,
        });
    }

    fn render_shape(&mut self, shape: ShapeHandle, transform: &Transform) {
        self.0.push(Command::RenderShape {
            shape,
            transform: transform.clone(),
        });
    }

    fn draw_rect(&mut self, color: Color, matrix: &Matrix) {
        self.0.push(Command::DrawRect {
            color,
            matrix: *matrix,
        });
    }

    fn push_mask(&mut self) {
        self.0.push(Command::PushMask);
    }

    fn activate_mask(&mut self) {
        self.0.push(Command::ActivateMask);
    }

    fn deactivate_mask(&mut self) {
        self.0.push(Command::DeactivateMask);
    }

    fn pop_mask(&mut self) {
        self.0.push(Command::PopMask);
    }

    fn blend(&mut self, commands: CommandList, blend_mode: BlendMode) {
        self.0.push(Command::Blend(commands, blend_mode));
    }
}

#[derive(Debug)]
pub enum Command {
    RenderBitmap {
        bitmap: BitmapHandle,
        transform: Transform,
        smoothing: bool,
    },
    RenderShape {
        shape: ShapeHandle,
        transform: Transform,
    },
    DrawRect {
        color: Color,
        matrix: Matrix,
    },
    PushMask,
    ActivateMask,
    DeactivateMask,
    PopMask,
    Blend(CommandList, BlendMode),
}
