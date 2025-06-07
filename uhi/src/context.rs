use glam::Vec2;

use crate::{
    DrawBuffer, DrawData, Externs, Fill, FillTexture, FontHandle, FontService, LineShape,
    RectShape, Rgba8, TextureKind, TextureService,
};

pub struct Context<E: Externs> {
    pub font_service: FontService,
    pub texture_service: TextureService<E>,

    draw_buffer: DrawBuffer<E>,
}

impl<E: Externs> Default for Context<E> {
    fn default() -> Self {
        Self {
            texture_service: TextureService::default(),
            font_service: FontService::default(),

            draw_buffer: DrawBuffer::default(),
        }
    }
}

impl<E: Externs> Context<E> {
    // draw buffer delegates

    #[inline]
    pub fn draw_line(&mut self, line: LineShape) {
        self.draw_buffer.push_line(line);
    }

    #[inline]
    pub fn draw_rect(&mut self, rect: RectShape<E>) {
        self.draw_buffer.push_rect(rect);
    }

    #[inline]
    pub fn get_draw_data<'a>(&'a self) -> DrawData<'a, E> {
        self.draw_buffer.get_draw_data()
    }

    #[inline]
    pub fn clear_draw_buffer(&mut self) {
        self.draw_buffer.clear();
    }

    // text

    pub fn draw_text(
        &mut self,
        text: &str,
        font_handle: FontHandle,
        font_size: f32,
        position: Vec2,
        color: Rgba8,
    ) {
        let font_ascent = self.font_service.get_font_ascent(font_handle, font_size);
        let mut x_offset = position.x;

        for ch in text.chars() {
            let char_ref =
                self.font_service
                    .get_char(ch, font_handle, font_size, &mut self.texture_service);
            let char_bounds = char_ref.bounds();
            let y_offset = position.y + font_ascent;

            self.draw_buffer.push_rect(RectShape::with_fill(
                char_bounds.translate_by(&Vec2::new(x_offset, y_offset)),
                Fill::new(
                    color,
                    FillTexture {
                        kind: TextureKind::Internal(char_ref.tex_handle()),
                        coords: char_ref.tex_coords(),
                    },
                ),
            ));

            x_offset += char_ref.advance_width();
        }
    }
}
