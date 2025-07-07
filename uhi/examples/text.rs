use app::AppHandler;
use gl::api::Apier as _;
use glam::Vec2;
use window::{Event, WindowAttrs, WindowEvent};

const FONT: &[u8] = include_bytes!("../../fixtures/JetBrainsMono-Regular.ttf");

struct UhiExterns;

impl uhi::Externs for UhiExterns {
    type TextureHandle = <uhi::GlRenderer as uhi::Renderer>::TextureHandle;
}

struct App {
    uhi_context: uhi::Context<UhiExterns>,
    uhi_renderer: uhi::GlRenderer,

    font_handle: uhi::FontHandle,
    input_state: input::State,

    text_one: String,
    text_one_state: uhi::TextState,

    text_two: String,
    text_two_state: uhi::TextState,

    text_appearance: uhi::TextAppearance,
}

impl AppHandler for App {
    fn create(ctx: app::AppContext) -> Self {
        let mut uhi_context = uhi::Context::default();
        let uhi_renderer = uhi::GlRenderer::new(ctx.gl_api).expect("uhi gl renderer fucky wucky");

        let font_handle = uhi_context
            .font_service
            .register_font_slice(FONT)
            .expect("invalid font");

        Self {
            uhi_context,
            uhi_renderer,

            font_handle,
            input_state: input::State::default(),

            text_one: "hello, sailor!".to_string(),
            text_one_state: uhi::TextState::default(),

            text_two: "こんにちは".to_string(),
            text_two_state: uhi::TextState::default(),

            text_appearance: uhi::TextAppearance::new(font_handle, 14.0),
        }
    }

    fn handle_event(&mut self, _ctx: app::AppContext, event: Event) {
        match event {
            Event::Window(WindowEvent::ScaleFactorChanged { scale_factor }) => {
                self.uhi_context
                    .font_service
                    .set_scale_factor(scale_factor, &mut self.uhi_context.texture_service);
            }
            Event::Pointer(ev) => {
                self.input_state.pointer.handle_event(ev);
            }
            Event::Keyboard(ev) => {
                self.input_state.keyboard.handle_event(ev);
            }
            _ => {}
        }
    }

    fn update(&mut self, ctx: app::AppContext) {
        self.uhi_context.interaction_state.begin_frame();

        // ----

        let window_size = ctx.window.size();

        unsafe { ctx.gl_api.clear_color(0.0, 0.0, 0.3, 1.0) };
        unsafe { ctx.gl_api.clear(gl::api::COLOR_BUFFER_BIT) };

        uhi::draw_readonly_text(
            &format!(
                "{:04}, {:04}",
                self.input_state.pointer.position.0.round(),
                self.input_state.pointer.position.1.round()
            ),
            &self.text_appearance,
            Vec2::new(24.0, 24.0 * 1.0),
            &mut self.uhi_context,
        );

        uhi::draw_editable_text(
            &mut self.text_one,
            &mut self.text_one_state,
            &self.text_appearance,
            Vec2::new(24.0, 24.0 * 3.0),
            &self.input_state,
            &mut self.uhi_context,
        );

        uhi::draw_editable_text(
            &mut self.text_two,
            &mut self.text_two_state,
            &self.text_appearance,
            Vec2::new(24.0, 24.0 * 5.0),
            &self.input_state,
            &mut self.uhi_context,
        );

        self.uhi_renderer
            .render(&mut self.uhi_context, ctx.gl_api, window_size)
            .expect("uhi renderer fucky wucky");

        // ----

        self.uhi_context.interaction_state.end_frame();
        self.uhi_context.draw_buffer.clear();

        self.input_state.end_frame();
    }
}

fn main() {
    app::run::<App>(WindowAttrs::default());
}
