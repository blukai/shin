use app::AppHandler;
use gl::api::Apier as _;
use window::{Event, WindowAttrs, WindowEvent};

struct GuiExterns;

impl gui::Externs for GuiExterns {
    type TextureHandle = <gui::GlRenderer as gui::Renderer>::TextureHandle;
}

struct App {
    gui_context: gui::Context<GuiExterns>,
    gui_renderer: gui::GlRenderer,
    input_state: input::State,

    rotation: f32,
    translation: gui::Vec2,
    scale: f32,
}

impl AppHandler for App {
    fn create(ctx: app::AppContext) -> Self {
        Self {
            gui_context: gui::Context::default(),
            gui_renderer: gui::GlRenderer::new(ctx.gl_api).expect("gui gl renderer fucky wucky"),
            input_state: input::State::default(),

            rotation: 0.0,
            translation: gui::Vec2::ZERO,
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
                self.gui_context
                    .font_service
                    .set_scale_factor(scale_factor as f32, &mut self.gui_context.texture_service);
            }

            Event::Pointer(input::PointerEvent::Pan {
                translation_delta, ..
            }) => {
                self.translation += gui::Vec2::from(gui::F64Vec2::from(translation_delta));
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
        self.gui_context.begin_frame();

        // ----

        unsafe { ctx.gl_api.clear_color(0.0, 0.0, 0.4, 1.0) };
        unsafe { ctx.gl_api.clear(gl::api::COLOR_BUFFER_BIT) };

        let physical_window_size = ctx.window.size();
        let scale_factor = ctx.window.scale_factor();
        let logical_window_size =
            gui::Vec2::from(gui::U32Vec2::from(physical_window_size)) / scale_factor as f32;

        gui::Text::new_non_interactive(
            format!(
                r#"
rotation:    {:.4} // TODO: support rotation
translation: x: {:.4}, y: {:.4}
scale:       {:.4}
                "#,
                self.rotation, self.translation.x, self.translation.y, self.scale,
            )
            .trim(),
            gui::Rect::new(gui::Vec2::ZERO, logical_window_size).shrink(&gui::Vec2::splat(16.0)),
        )
        .multiline()
        .draw(&mut self.gui_context);

        let center = logical_window_size / 2.0;
        let size = 100.0 * self.scale;
        let rect = gui::Rect::from_center_size(center, size).translate_by(&self.translation);
        self.gui_context
            .draw_buffer
            .push_rect(gui::RectShape::new_with_fill(
                rect,
                gui::Fill::new_with_color(gui::Rgba8::FUCHSIA),
            ));

        if let Some(cursor_shape) = self.gui_context.interaction_state.take_cursor_shape() {
            ctx.window
                .set_cursor_shape(cursor_shape)
                // TODO: proper error handling
                .expect("could not set cursor shape");
        }

        self.gui_renderer
            .render(
                &mut self.gui_context,
                ctx.gl_api,
                physical_window_size,
                scale_factor as f32,
            )
            // TODO: proper error handling
            .expect("gui renderer fucky wucky");

        // ----

        self.gui_context.end_frame();
        self.input_state.end_frame();
    }
}

fn main() {
    app::run::<App>(WindowAttrs::default());
}
