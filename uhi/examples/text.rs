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

            text_singleline_editable: "editable".to_string(),
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

        unsafe { ctx.gl_api.clear_color(0.0, 0.0, 0.8, 1.0) };
        unsafe { ctx.gl_api.clear(gl::api::COLOR_BUFFER_BIT) };

        let physical_window_size = ctx.window.size();
        let scale_factor = ctx.window.scale_factor();
        let logical_window_rect = uhi::Rect::new(
            uhi::Vec2::ZERO,
            uhi::Vec2::from(uhi::U32Vec2::from(physical_window_size)) / scale_factor as f32,
        );

        uhi::Text::new(
            format!(
                "pointer position: {:04}, {:04}.",
                self.input_state.pointer.position.0.round(),
                self.input_state.pointer.position.1.round()
            )
            .as_str(),
            logical_window_rect.shrink(&uhi::Vec2::new(16.0, 16.0 * 1.0)),
        )
        .singleline()
        .draw(&mut self.uhi_context);

        uhi::Text::new(
            "こんにちは",
            logical_window_rect.shrink(&uhi::Vec2::new(16.0, 16.0 * 3.0)),
        )
        .singleline()
        .selectable(&mut self.text_singleline_selection)
        .maybe_set_hot_or_active(
            uhi::Key::from_location(),
            &mut self.uhi_context,
            &self.input_state,
        )
        .update_if(|t| t.is_active(), &mut self.uhi_context, &self.input_state)
        .draw(&mut self.uhi_context);

        uhi::Text::new(
            &mut self.text_singleline_editable,
            logical_window_rect.shrink(&uhi::Vec2::new(16.0, 16.0 * 5.0)),
        )
        .singleline()
        .editable(&mut self.text_singleline_editable_selection)
        .maybe_set_hot_or_active(
            uhi::Key::from_location(),
            &mut self.uhi_context,
            &self.input_state,
        )
        .update_if(|t| t.is_active(), &mut self.uhi_context, &self.input_state)
        .draw(&mut self.uhi_context);

        uhi::Text::new(
            "With no bamboo hat\nDoes the drizzle fall on me?\nWhat care I of that?",
            logical_window_rect.shrink(&uhi::Vec2::new(16.0, 16.0 * 7.0)),
        )
        .multiline()
        .selectable(&mut self.text_multiline_selection)
        .maybe_set_hot_or_active(
            uhi::Key::from_location(),
            &mut self.uhi_context,
            &self.input_state,
        )
        .update_if(|t| t.is_active(), &mut self.uhi_context, &self.input_state)
        .draw(&mut self.uhi_context);

        self.uhi_renderer
            .render(
                &mut self.uhi_context,
                ctx.gl_api,
                physical_window_size,
                scale_factor as f32,
            )
            .expect("uhi renderer fucky wucky");

        // ----

        self.uhi_context.end_frame();
        self.input_state.end_frame();
    }
}

fn main() {
    app::run::<App>(WindowAttrs::default());
}
