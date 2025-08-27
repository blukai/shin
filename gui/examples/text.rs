use anyhow::Context as _;
use app::AppHandler;
use gl::api::Apier as _;
use window::{Event, WindowAttrs};

struct GuiExterns;

impl gui::Externs for GuiExterns {
    type TextureHandle = <gui::GlRenderer as gui::Renderer>::TextureHandle;
}

struct App {
    gui_context: gui::Context<GuiExterns>,
    gui_viewport: gui::Viewport<GuiExterns>,
    gui_renderer: gui::GlRenderer,

    input_state: input::State,

    text_singleline_state: gui::TextState,

    text_singleline_editable: String,
    text_singleline_editable_state: gui::TextState,

    text_multiline_state: gui::TextState,
}

impl AppHandler for App {
    fn create(ctx: app::AppContext) -> Self {
        Self {
            gui_context: gui::Context::default(),
            gui_viewport: gui::Viewport::default(),
            gui_renderer: gui::GlRenderer::new(ctx.gl_api).expect("gui gl renderer fucky wucky"),

            input_state: input::State::default(),

            text_singleline_state: gui::TextState::default(),

            text_singleline_editable: "hello, sailor".to_string(),
            text_singleline_editable_state: gui::TextState::default(),

            text_multiline_state: gui::TextState::default(),
        }
    }

    fn handle_event(&mut self, _ctx: app::AppContext, event: Event) {
        match event {
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
        let physical_size = gui::Vec2::from(gui::U32Vec2::from(ctx.window.physical_size()));
        let scale_factor = ctx.window.scale_factor() as f32;

        self.input_state.begin_iteration();
        self.gui_context.begin_iteration();
        self.gui_viewport.begin_frame(physical_size, scale_factor);

        // ----

        unsafe { ctx.gl_api.clear_color(0.0, 0.0, 0.4, 1.0) };
        unsafe { ctx.gl_api.clear(gl::api::COLOR_BUFFER_BIT) };

        let logical_size = physical_size / scale_factor;
        let logical_rect = gui::Rect::new(gui::Vec2::ZERO, logical_size);

        let primary_text_appearance =
            gui::TextAppearance::from_appearance(&self.gui_context.appearance);
        let caption_text_appearance =
            gui::TextAppearance::from_appearance(&self.gui_context.appearance)
                .with_font_size(primary_text_appearance.font_size * 0.8)
                .with_fg(gui::Rgba8::GRAY);

        // TODO: automatic layout or something
        let font_height_factor = self
            .gui_context
            .font_service
            .get_or_create_font_instance(
                self.gui_context.appearance.font_handle,
                self.gui_context.appearance.font_size,
                self.gui_viewport.scale_factor,
            )
            .height()
            / self.gui_context.appearance.font_size;
        let mut rect = logical_rect.inflate(-gui::Vec2::splat(16.0));
        let mut use_rect = |font_size: f32, times: usize, add_gap: bool| -> gui::Rect {
            let prev = rect;
            let font_height = font_size * font_height_factor;
            rect.min.y += font_height * times as f32;
            if add_gap {
                rect.min.y += 8.0;
            }
            prev
        };

        {
            gui::Text::new_non_interactive(
                "singleline non-selectable and non-editable:",
                use_rect(caption_text_appearance.font_size, 1, false),
            )
            .with_appearance(caption_text_appearance.clone())
            .singleline()
            .draw(&mut self.gui_context, &mut self.gui_viewport);

            let (x, y) = self.input_state.pointer.position;
            gui::Text::new_non_interactive(
                format!("x: {}, y: {}", x.round(), y.round()).as_str(),
                use_rect(primary_text_appearance.font_size, 1, true),
            )
            .with_appearance(primary_text_appearance.clone())
            .singleline()
            .draw(&mut self.gui_context, &mut self.gui_viewport);
        }

        {
            gui::Text::new_non_interactive(
                "singleline selectable:",
                use_rect(caption_text_appearance.font_size, 1, false),
            )
            .with_appearance(caption_text_appearance.clone())
            .singleline()
            .draw(&mut self.gui_context, &mut self.gui_viewport);

            gui::Text::new_selectable(
                "なかなか興味深いですね",
                use_rect(primary_text_appearance.font_size, 1, true),
                &mut self.text_singleline_state,
            )
            .with_appearance(primary_text_appearance.clone())
            .singleline()
            .draw(
                &mut self.gui_context,
                &mut self.gui_viewport,
                &self.input_state,
            );
        }

        {
            gui::Text::new_non_interactive(
                "singleline editable:",
                use_rect(caption_text_appearance.font_size, 1, false),
            )
            .with_appearance(caption_text_appearance.clone())
            .singleline()
            .draw(&mut self.gui_context, &mut self.gui_viewport);

            gui::Text::new_editable(
                &mut self.text_singleline_editable,
                use_rect(primary_text_appearance.font_size, 1, true),
                &mut self.text_singleline_editable_state,
            )
            .with_appearance(primary_text_appearance.clone())
            .singleline()
            .draw(
                &mut self.gui_context,
                &mut self.gui_viewport,
                &self.input_state,
            );
        }

        {
            gui::Text::new_non_interactive(
                "multiline selectable:",
                use_rect(caption_text_appearance.font_size, 1, false),
            )
            .with_appearance(caption_text_appearance.clone())
            .singleline()
            .draw(&mut self.gui_context, &mut self.gui_viewport);

            gui::Text::new_selectable(
                "With no bamboo hat\nDoes the drizzle fall on me?\nWhat care I of that?",
                use_rect(primary_text_appearance.font_size, 3, true),
                &mut self.text_multiline_state,
            )
            .with_appearance(primary_text_appearance.clone())
            .multiline()
            .draw(
                &mut self.gui_context,
                &mut self.gui_viewport,
                &self.input_state,
            );
        }

        // TODO: need scroll area
        {
            gui::Text::new_non_interactive(
                "atlas:",
                use_rect(caption_text_appearance.font_size, 1, false),
            )
            .with_appearance(caption_text_appearance.clone())
            .singleline()
            .draw(&mut self.gui_context, &mut self.gui_viewport);
            rect.min.y += 4.0;

            for font_instance in self.gui_context.font_service.iter_font_instances() {
                for texture_page in font_instance.iter_texture_pages() {
                    let size = gui::Vec2::from(gui::U32Vec2::from(
                        texture_page.texture_packer.texture_size(),
                    ));
                    self.gui_viewport
                        .draw_buffer
                        .push_rect(gui::RectShape::new_with_fill(
                            gui::Rect::new(rect.min, rect.min + size),
                            gui::Fill::new(
                                gui::Rgba8::WHITE,
                                gui::FillTexture {
                                    kind: gui::TextureKind::Internal(texture_page.texture_handle),
                                    coords: gui::Rect::from_center_half_size(
                                        gui::Vec2::splat(0.5),
                                        1.0,
                                    ),
                                },
                            ),
                        ));

                    // shift next texture to the right
                    rect.min.x += size.x;
                }
            }
        }

        if let Some(cursor_shape) = self.gui_context.interaction_state.take_cursor_shape() {
            ctx.window
                .set_cursor_shape(cursor_shape)
                // TODO: proper error handling
                .expect("could not set cursor shape");
        }

        if self.gui_context.clipboard_state.is_awaiting_read() {
            // TODO: figure out maybe how to incorporate reusable clipboard-read buffer into gui
            // context or something?
            let mut buf = vec![];
            let payload = ctx
                .window
                .read_clipboard(window::MIME_TYPE_TEXT, &mut buf)
                .and_then(|_| String::from_utf8(buf).context("invalid text"));
            self.gui_context.clipboard_state.fulfill_read(payload);
        }

        if let Some(text) = self.gui_context.clipboard_state.take_write() {
            ctx.window
                .provide_clipboard_data(Box::new(window::ClipboardTextProvider::new(text)))
                // TODO: proper error handling
                .expect("could not provive clipboard data");
        }

        self.gui_renderer
            .render(&mut self.gui_context, &mut self.gui_viewport, ctx.gl_api)
            // TODO: proper error handling
            .expect("gui renderer fucky wucky");

        // ----

        self.gui_viewport.end_frame();
        self.gui_context.end_iteration();
        self.input_state.end_iteration();
    }
}

fn main() {
    app::run::<App>(WindowAttrs::default());
}
