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

    rotation: f32,
    translation: uhi::Vec2,
    scale: f32,
}

impl AppHandler for App {
    fn create(ctx: app::AppContext) -> Self {
        Self {
            uhi_context: uhi::Context::default(),
            uhi_renderer: uhi::GlRenderer::new(ctx.gl_api).expect("uhi gl renderer fucky wucky"),
            input_state: input::State::default(),

            rotation: 0.0,
            translation: uhi::Vec2::ZERO,
            scale: 1.0,
        }
    }

    fn handle_event(&mut self, _ctx: app::AppContext, event: Event) {
        // this is ugly

        match event {
            Event::Pointer(ref ev) => {
                self.input_state
                    .handle_event(input::Event::Pointer(ev.clone()));
            }
            Event::Keyboard(ref ev) => {
                self.input_state
                    .handle_event(input::Event::Keyboard(ev.clone()));
            }

            _ => {}
        }

        match event {
            Event::Window(WindowEvent::ScaleFactorChanged { scale_factor }) => {
                self.uhi_context
                    .font_service
                    .set_scale_factor(scale_factor as f32, &mut self.uhi_context.texture_service);
            }

            Event::Pointer(input::PointerEvent::Pan {
                translation_delta, ..
            }) => {
                self.translation += uhi::Vec2::from(uhi::F64Vec2::from(translation_delta));
            }
            Event::Pointer(input::PointerEvent::Zoom { scale_delta, .. }) => {
                self.scale += scale_delta as f32;
            }
            Event::Pointer(input::PointerEvent::Rotate { rotation_delta, .. }) => {
                self.rotation += rotation_delta as f32;
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
        let logical_window_size =
            uhi::Vec2::from(uhi::U32Vec2::from(physical_window_size)) / scale_factor as f32;

        uhi::Text::new_non_interactive(
            format!(
                r#"
rotation:    {:.4} // TODO: support rotation
translation: x: {:.4}, y: {:.4}
scale:       {:.4}
                "#,
                self.rotation, self.translation.x, self.translation.y, self.scale,
            )
            .trim(),
            uhi::Rect::new(uhi::Vec2::ZERO, logical_window_size).shrink(&uhi::Vec2::splat(16.0)),
        )
        .multiline()
        .draw(&mut self.uhi_context);

        let center = logical_window_size / 2.0;
        let size = 100.0 * self.scale;
        let rect = uhi::Rect::from_center_size(center, size).translate_by(&self.translation);
        self.uhi_context
            .draw_buffer
            .push_rect(uhi::RectShape::new_with_fill(
                rect,
                uhi::Fill::new_with_color(uhi::Rgba8::FUCHSIA),
            ));

        if let Some(cursor_shape) = self.uhi_context.interaction_state.take_cursor_shape() {
            ctx.window
                .set_cursor_shape(cursor_shape)
                // TODO: proper error handling
                .expect("could not set cursor shape");
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
