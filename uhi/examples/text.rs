use anyhow::Context as _;
use app::AppHandler;
use gl::api::Apier as _;
use window::{Event, WindowAttrs, WindowEvent};

struct UhiExterns;

impl uhi::Externs for UhiExterns {
    type TextureHandle = <uhi::GlRenderer as uhi::Renderer>::TextureHandle;
}

struct App {
    uhi_context: uhi::Context<UhiExterns>,
    uhi_renderer: uhi::GlRenderer,

    input_state: input::State,

    text_singleline_selection: uhi::TextSelection,

    text_singleline_editable: String,
    text_singleline_editable_selection: uhi::TextSelection,

    text_multiline_selection: uhi::TextSelection,
}

impl AppHandler for App {
    fn create(ctx: app::AppContext) -> Self {
        Self {
            uhi_context: uhi::Context::default(),
            uhi_renderer: uhi::GlRenderer::new(ctx.gl_api).expect("uhi gl renderer fucky wucky"),

            input_state: input::State::default(),

            text_singleline_selection: uhi::TextSelection::default(),

            text_singleline_editable: "hello, sailor".to_string(),
            text_singleline_editable_selection: uhi::TextSelection::default(),

            text_multiline_selection: uhi::TextSelection::default(),
        }
    }

    fn handle_event(&mut self, _ctx: app::AppContext, event: Event) {
        match event {
            Event::Window(WindowEvent::ScaleFactorChanged { scale_factor }) => {
                self.uhi_context
                    .font_service
                    .set_scale_factor(scale_factor as f32, &mut self.uhi_context.texture_service);
            }
            Event::Pointer(ev) => {
                self.input_state.handle_event(input::Event::Pointer(ev));
            }
            Event::Keyboard(ev) => {
                self.input_state.handle_event(input::Event::Keyboard(ev));
            }
            _ => {}
        }
    }

    fn update(&mut self, ctx: app::AppContext) {
        self.uhi_context.begin_frame();

        // ----

        unsafe { ctx.gl_api.clear_color(0.0, 0.0, 0.4, 1.0) };
        unsafe { ctx.gl_api.clear(gl::api::COLOR_BUFFER_BIT) };

        let physical_window_size = ctx.window.size();
        let scale_factor = ctx.window.scale_factor();
        let logical_window_rect = uhi::Rect::new(
            uhi::Vec2::ZERO,
            uhi::Vec2::from(uhi::U32Vec2::from(physical_window_size)) / scale_factor as f32,
        );

        let primary_font_size = self.uhi_context.default_font_size();
        let caption_font_size = primary_font_size * 0.8;
        let caption_text_palette = uhi::TextPalette::default().with_fg(uhi::Rgba8::GRAY);

        // TODO: automatic layout or something
        let font_height_factor = self
            .uhi_context
            .font_service
            .get_font_instance(
                self.uhi_context.default_font_handle(),
                self.uhi_context.default_font_size(),
            )
            .height()
            / self.uhi_context.default_font_size();
        let mut rect = logical_window_rect.shrink(&uhi::Vec2::splat(16.0));
        let mut use_rect = |font_size: f32, times: usize, add_gap: bool| -> uhi::Rect {
            let prev = rect;
            let font_height = font_size * font_height_factor;
            rect.min.y += font_height * times as f32;
            if add_gap {
                rect.min.y += 8.0;
            }
            prev
        };

        {
            uhi::Text::new(
                "singleline non-selectable and non-editable:",
                use_rect(caption_font_size, 1, false),
            )
            .with_font_size(caption_font_size)
            .with_palette(caption_text_palette.clone())
            .singleline()
            .draw(&mut self.uhi_context);

            let (x, y) = self.input_state.pointer.position;
            uhi::Text::new(
                format!("x: {}, y: {}", x.round(), y.round()).as_str(),
                use_rect(primary_font_size, 1, true),
            )
            .with_font_size(primary_font_size)
            .singleline()
            .draw(&mut self.uhi_context);
        }

        {
            uhi::Text::new(
                "singleline selectable:",
                use_rect(caption_font_size, 1, false),
            )
            .with_font_size(caption_font_size)
            .with_palette(caption_text_palette.clone())
            .singleline()
            .draw(&mut self.uhi_context);

            let key = uhi::Key::from_location();
            uhi::Text::new(
                "なかなか興味深いですね",
                use_rect(primary_font_size, 1, true),
            )
            .with_font_size(primary_font_size)
            .singleline()
            .selectable(&mut self.text_singleline_selection)
            .maybe_set_hot_or_active(key, &mut self.uhi_context, &self.input_state)
            .update_if(
                |t| t.is_active(),
                key,
                &mut self.uhi_context,
                &self.input_state,
            )
            .draw(&mut self.uhi_context);
        }

        {
            uhi::Text::new(
                "singleline editable:",
                use_rect(caption_font_size, 1, false),
            )
            .with_font_size(caption_font_size)
            .with_palette(caption_text_palette.clone())
            .singleline()
            .draw(&mut self.uhi_context);

            let key = uhi::Key::from_location();
            uhi::Text::new(
                &mut self.text_singleline_editable,
                use_rect(primary_font_size, 1, true),
            )
            .with_font_size(primary_font_size)
            .singleline()
            .editable(&mut self.text_singleline_editable_selection)
            .maybe_set_hot_or_active(key, &mut self.uhi_context, &self.input_state)
            .update_if(
                |t| t.is_active(),
                key,
                &mut self.uhi_context,
                &self.input_state,
            )
            .draw(&mut self.uhi_context);
        }

        {
            uhi::Text::new(
                "multiline selectable:",
                use_rect(caption_font_size, 1, false),
            )
            .with_font_size(caption_font_size)
            .with_palette(caption_text_palette.clone())
            .singleline()
            .draw(&mut self.uhi_context);

            let key = uhi::Key::from_location();
            uhi::Text::new(
                "With no bamboo hat\nDoes the drizzle fall on me?\nWhat care I of that?",
                use_rect(primary_font_size, 3, true),
            )
            .with_font_size(primary_font_size)
            .multiline()
            .selectable(&mut self.text_multiline_selection)
            .maybe_set_hot_or_active(key, &mut self.uhi_context, &self.input_state)
            .update_if(
                |t| t.is_active(),
                key,
                &mut self.uhi_context,
                &self.input_state,
            )
            .draw(&mut self.uhi_context);
        }

        {
            uhi::Text::new("multiline editable:", use_rect(caption_font_size, 1, false))
                .with_font_size(caption_font_size)
                .with_palette(caption_text_palette.clone())
                .singleline()
                .draw(&mut self.uhi_context);

            uhi::Text::new("TODO", use_rect(primary_font_size, 1, true))
                .with_font_size(primary_font_size)
                .with_palette(uhi::TextPalette::default().with_fg(uhi::Rgba8::RED))
                .singleline()
                .draw(&mut self.uhi_context);
        }

        // TODO: need scroll area
        {
            uhi::Text::new("atlas:", use_rect(caption_font_size, 1, false))
                .with_font_size(caption_font_size)
                .with_palette(caption_text_palette.clone())
                .singleline()
                .draw(&mut self.uhi_context);
            rect.min.y += 4.0;

            for tp in self.uhi_context.font_service.iter_texture_pages() {
                let size = uhi::Vec2::from(uhi::U32Vec2::from(tp.size())) / scale_factor as f32;
                self.uhi_context
                    .draw_buffer
                    .push_rect(uhi::RectShape::with_fill(
                        uhi::Rect::new(rect.min, rect.min + size),
                        uhi::Fill::new(
                            uhi::Rgba8::WHITE,
                            uhi::FillTexture {
                                kind: uhi::TextureKind::Internal(tp.handle()),
                                coords: uhi::Rect::from_center_size(uhi::Vec2::splat(0.5), 1.0),
                            },
                        ),
                    ));
            }
        }

        if let Some(cursor_shape) = self.uhi_context.take_cursor_shape() {
            ctx.window
                .set_cursor_shape(cursor_shape)
                // TODO: proper error handling
                .expect("could not set cursor shape");
        }

        if let Some(clipboard_read) = self.uhi_context.get_pending_clipboard_read_mut() {
            // TODO: figure out maybe how to incorporate reusable clipboard-read buffer into uhi
            // context or something?
            let mut buf = vec![];
            let payload = ctx
                .window
                .read_clipboard(window::MIME_TYPE_TEXT, &mut buf)
                .and_then(|_| String::from_utf8(buf).context("invalid text"));
            clipboard_read.fulfill(payload);
        }

        if let Some(text) = self.uhi_context.take_pending_clipboard_write() {
            ctx.window
                .provide_clipboard_data(Box::new(window::ClipboardTextProvider::new(text)))
                // TODO: proper error handling
                .expect("could not provive clipboard data");
        }

        self.uhi_renderer
            .render(
                &mut self.uhi_context,
                ctx.gl_api,
                physical_window_size,
                scale_factor as f32,
            )
            // TODO: proper error handling
            .expect("uhi renderer fucky wucky");

        // ----

        self.uhi_context.end_frame();
        self.input_state.end_frame();
    }
}

fn main() {
    app::run::<App>(WindowAttrs::default());
}
