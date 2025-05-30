pub use fontdue::layout::{
    HorizontalAlign as TextHAlign, LayoutSettings as TextLayoutSttings,
    VerticalAlign as TextVAlign, WrapStyle as TextWrapStyle,
};
use fontdue::layout::{Layout as TextLayout, TextStyle};
use glam::Vec2;

use crate::{
    DrawBuffer, Fill, FillTexture, FontHandle, FontService, LineShape, Rect, RectShape, Renderer,
    Rgba8, TextureKind, TextureService,
};

// TODO: context must own renderer, but renderer will need its own context (args) or something.
pub struct Context<R: Renderer> {
    font_service: FontService,
    // TODO: unexpose
    pub texture_service: TextureService<R>,
    text_layout: fontdue::layout::Layout,
    // TODO: unexpose
    pub draw_buffer: DrawBuffer<R>,
}

impl<R: Renderer> Context<R> {
    pub fn new(yup: bool) -> Self {
        Self {
            draw_buffer: DrawBuffer::default(),
            texture_service: TextureService::default(),
            font_service: FontService::default(),
            text_layout: TextLayout::new({
                use fontdue::layout::CoordinateSystem::*;
                if yup { PositiveYUp } else { PositiveYDown }
            }),
        }
    }

    #[inline]
    pub fn create_font<D>(&mut self, data: D, size: f32) -> anyhow::Result<FontHandle>
    where
        D: AsRef<[u8]>,
    {
        self.font_service.create_font(data, size)
    }

    #[inline]
    pub fn push_line(&mut self, line: LineShape) {
        self.draw_buffer.push_line(line);
    }

    #[inline]
    pub fn push_rect(&mut self, rect: RectShape<R>) {
        self.draw_buffer.push_rect(rect);
    }

    pub fn push_text(
        &mut self,
        font_handle: FontHandle,
        text: &str,
        color: Rgba8,
        maybe_settings: Option<&TextLayoutSttings>,
    ) {
        let font = self.font_service.get_font(font_handle);

        if let Some(settings) = maybe_settings {
            self.text_layout.reset(settings);
        } else {
            self.text_layout.clear();
        }

        self.text_layout
            .append(&[&font.fontdue_font], &TextStyle::new(text, font.size, 0));
        for glyph in self.text_layout.glyphs() {
            self.draw_buffer.push_rect(RectShape::with_fill(
                {
                    let min = Vec2::new(glyph.x, glyph.y);
                    let size = Vec2::new(glyph.width as f32, glyph.height as f32);
                    Rect::new(min, min + size)
                },
                {
                    let (tex_handle, tex_coords) =
                        self.font_service.get_or_create_texture_for_char(
                            font_handle,
                            glyph.parent,
                            &mut self.texture_service,
                        );
                    Fill::new(
                        color,
                        FillTexture {
                            kind: TextureKind::Internal(tex_handle),
                            coords: tex_coords,
                        },
                    )
                },
            ));
        }
    }
}
