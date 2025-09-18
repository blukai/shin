use gl::Apier as _;
use window::{Event, WindowAttrs};

use example::{Context, Handler, RendererGl as Renderer, run};

const DEFAULT_FONT_DATA: &[u8] = include_bytes!("../fixtures/JetBrainsMono-Regular.ttf");

fn draw_text<E: sx::Externs>(
    text: &str,
    mut font_instance: sx::FontInstanceRefMut,
    fg: sx::Rgba,
    position: sx::Vec2,
    texture_service: &mut sx::TextureService,
    draw_buffer: &mut sx::DrawBuffer<E>,
) {
    let font_ascent = font_instance.ascent();
    let mut x_offset: f32 = position.x;
    for ch in text.chars() {
        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);
        let glyph_advance_width = glyph.advance_width();
        draw_buffer.push_rect(sx::RectShape::new_with_fill(
            glyph
                .bounding_rect()
                .translate(sx::Vec2::new(x_offset, position.y + font_ascent)),
            sx::Fill::new(
                fg,
                sx::FillTexture {
                    texture: sx::TextureHandleKind::Internal(glyph.texture_handle()),
                    coords: glyph.texture_coords(),
                },
            ),
        ));
        x_offset += glyph_advance_width;
    }
}

struct App {
    texture_service: sx::TextureService,
    font_service: sx::FontService,
    default_font_handle: sx::FontHandle,
    draw_buffer: sx::DrawBuffer<Renderer>,
    renderer: Renderer,
    input: input::State,
}

impl Handler for App {
    fn create(ctx: Context) -> Self {
        let mut font_service = sx::FontService::default();
        let default_font_handle = font_service
            .register_font_slice(DEFAULT_FONT_DATA)
            // NOTE: am i okay with paniching here because the panic may only be caused by an
            // invalid font file; you can guarantee valitidy of by not putting an invalid default
            // font into fixtures directory xd.
            .expect("somebody fucked things up; default font is invalid?");

        Self {
            texture_service: sx::TextureService::default(),
            font_service,
            default_font_handle,
            draw_buffer: sx::DrawBuffer::default(),
            renderer: Renderer::new(ctx.gl_api).expect("gl renderer fucky wucky"),
            input: input::State::default(),
        }
    }

    fn iterate(&mut self, ctx: Context, events: impl Iterator<Item = Event>) {
        self.draw_buffer.clear();
        self.font_service
            .remove_unused_font_instances(&mut self.texture_service);
        self.input
            .handle_events(events.filter_map(|event| match event {
                Event::Window(_) => None,
                Event::Pointer(pointer_event) => Some(input::Event::Pointer(pointer_event)),
                Event::Keyboard(keyboard_event) => Some(input::Event::Keyboard(keyboard_event)),
            }));

        // ----

        unsafe { ctx.gl_api.clear_color(0.0, 0.0, 0.4, 1.0) };
        unsafe { ctx.gl_api.clear(gl::COLOR_BUFFER_BIT) };

        let scale_factor = ctx.window.scale_factor() as f32;
        let logical_size = sx::Vec2::from(sx::U32Vec2::from(ctx.window.logical_size()));
        let logical_rect = sx::Rect::new(sx::Vec2::ZERO, logical_size);

        let font_instance = self.font_service.get_or_create_font_instance(
            self.default_font_handle,
            16.0,
            scale_factor,
        );
        draw_text(
            "hello sailor!",
            font_instance,
            sx::Rgba::WHITE,
            logical_rect.min + sx::Vec2::splat(24.0),
            &mut self.texture_service,
            &mut self.draw_buffer,
        );

        self.renderer
            .handle_texture_commands(self.texture_service.drain_comands(), &ctx.gl_api)
            .expect("could not update textures");
        self.renderer
            .render(
                logical_size,
                scale_factor,
                &mut self.draw_buffer,
                ctx.gl_api,
            )
            // TODO: proper error handling
            .expect("renderer fucky wucky");
    }
}

fn main() {
    run::<App>(WindowAttrs::default());
}
