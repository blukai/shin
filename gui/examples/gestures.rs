use app::AppHandler;
use gl::Apier as _;
use window::{Event, WindowAttrs};

struct GuiExterns;

impl gui::Externs for GuiExterns {
    type TextureHandle = <gui::GlRenderer as gui::Renderer>::TextureHandle;
}

struct App {
    gui_context: gui::Context,
    gui_viewport: gui::Viewport<GuiExterns>,
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
            gui_viewport: gui::Viewport::default(),
            gui_renderer: gui::GlRenderer::new(ctx.gl_api).expect("gui gl renderer fucky wucky"),
            input_state: input::State::default(),

            rotation: 0.0,
            translation: gui::Vec2::ZERO,
            scale: 1.0,
        }
    }

    fn iterate(&mut self, ctx: app::AppContext, events: impl Iterator<Item = Event>) {
        let physical_size = gui::Vec2::from(gui::U32Vec2::from(ctx.window.physical_size()));
        let scale_factor = ctx.window.scale_factor() as f32;

        self.input_state
            .begin_iteration(events.filter_map(|event| match event {
                Event::Window(_) => None,
                Event::Pointer(pointer_event) => {
                    // TODO: this is ugly
                    match pointer_event.kind {
                        input::PointerEventKind::Pan {
                            translation_delta, ..
                        } => {
                            self.translation +=
                                gui::Vec2::from(gui::F64Vec2::from(translation_delta));
                        }
                        input::PointerEventKind::Zoom { scale_delta, .. } => {
                            self.scale += scale_delta as f32;
                        }
                        input::PointerEventKind::Rotate { rotation_delta, .. } => {
                            self.rotation += rotation_delta as f32;
                        }
                        _ => {}
                    }

                    Some(input::Event::Pointer(pointer_event))
                }
                Event::Keyboard(keyboard_event) => Some(input::Event::Keyboard(keyboard_event)),
            }));
        self.gui_context.begin_iteration(&self.input_state);
        self.gui_viewport.begin_frame(physical_size, scale_factor);

        // ----

        unsafe { ctx.gl_api.clear_color(0.1, 0.2, 0.4, 1.0) };
        unsafe { ctx.gl_api.clear(gl::COLOR_BUFFER_BIT) };

        let logical_size = physical_size / scale_factor;
        let logical_rect = gui::Rect::new(gui::Vec2::ZERO, logical_size);

        let center = logical_size / 2.0;
        let size = 100.0 * self.scale;
        let rect = gui::Rect::from_center_half_size(center, size).translate(self.translation);
        self.gui_viewport
            .draw_buffer
            .push_rect(gui::RectShape::new_with_fill(
                rect,
                gui::Fill::new_with_color(gui::Rgba::ORANGE),
            ));

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
            logical_rect.inflate(-gui::Vec2::splat(16.0)),
        )
        .multiline()
        .draw(&mut self.gui_context, &mut self.gui_viewport);

        if let Some(cursor_shape) = self.gui_context.interaction_state.take_cursor_shape() {
            ctx.window
                .set_cursor_shape(cursor_shape)
                // TODO: proper error handling
                .expect("could not set cursor shape");
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
