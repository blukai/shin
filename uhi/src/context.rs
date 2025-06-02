pub use fontdue::layout::{
    GlyphPosition, HorizontalAlign as TextHAlign, LayoutSettings as TextLayoutSttings,
    VerticalAlign as TextVAlign, WrapStyle as TextWrapStyle,
};
use glam::Vec2;

use crate::{
    DrawBuffer, DrawData, Fill, FillTexture, FontHandle, FontService, LineShape, Rect, RectShape,
    Renderer, Rgba8, TextureKind, TextureService,
};

pub struct Context<R: Renderer> {
    pub font_service: FontService,
    pub texture_service: TextureService<R>,

    draw_buffer: DrawBuffer<R>,
}

impl<R: Renderer> Default for Context<R> {
    fn default() -> Self {
        Self {
            draw_buffer: DrawBuffer::default(),
            texture_service: TextureService::default(),
            font_service: FontService::default(),
        }
    }
}

impl<R: Renderer> Context<R> {
    // draw buffer delegates

    #[inline]
    pub fn draw_line(&mut self, line: LineShape) {
        self.draw_buffer.push_line(line);
    }

    #[inline]
    pub fn draw_rect(&mut self, rect: RectShape<R>) {
        self.draw_buffer.push_rect(rect);
    }

    #[inline]
    pub fn get_draw_data<'a>(&'a self) -> DrawData<'a, R> {
        self.draw_buffer.get_draw_data()
    }

    #[inline]
    pub fn clear_draw_buffer(&mut self) {
        self.draw_buffer.clear();
    }

    // text

    pub fn draw_text(&mut self, text: &str, font_handle: FontHandle, position: Vec2, color: Rgba8) {
        let mut x = position.x;
        for ch in text.chars() {
            let ch =
                self.font_service
                    .get_or_allocate_char(ch, font_handle, &mut self.texture_service);

            let metrics = ch.metrics();
            let size = Vec2::new(metrics.width as f32, metrics.height as f32);
            let min = Vec2::new(
                x + metrics.bounds.xmin,
                position.y + ch.font_ascent() - (metrics.bounds.ymin + metrics.bounds.height),
            );
            let max = min + size;
            x += metrics.advance_width;

            self.draw_buffer.push_rect(RectShape::with_fill(
                Rect::new(min, max),
                Fill::new(
                    color,
                    FillTexture {
                        kind: TextureKind::Internal(ch.tex_handle()),
                        coords: ch.tex_coords(),
                    },
                ),
            ));
        }
    }
}
