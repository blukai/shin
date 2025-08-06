use anyhow::Context as _;
use app::AppHandler;
use gl::api::Apier as _;
use window::{Event, WindowAttrs, WindowEvent};

struct UhiExterns;

impl uhi::Externs for UhiExterns {
    type TextureHandle = <uhi::GlRenderer as uhi::Renderer>::TextureHandle;
}

// TODO: can this be turned into something reusable and possibly generic(think of animating
// colors)?
//
// TODO: how animations should behave in a "power saving mode" when re-renderes happen only in
// response to the user input / not in continuous, but in reactive mode? should there be a way to
// request a redraw or something?
#[derive(Default)]
struct Animation {
    from: f32,
    to: f32,
    // TODO: would it make sense to introduce Timer as a separate thing?
    duration: f32,
    elapsed: f32,
    just_finished: bool,
}

impl Animation {
    fn start(&mut self, from: f32, to: f32, duration: f32) {
        self.from = from;
        self.to = to;
        self.duration = duration;
        self.elapsed = 0.0;
        self.just_finished = false;
    }

    fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    fn just_finished(&self) -> bool {
        self.just_finished
    }

    fn maybe_step(&mut self, dt: f32) {
        if self.is_finished() {
            self.just_finished = false;
            return;
        }

        let prev_finished = self.is_finished();
        self.elapsed += dt;
        let next_finished = self.is_finished();
        self.just_finished = !prev_finished && next_finished;
    }

    fn get_value(&self) -> f32 {
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * t
    }

    // TODO: maybe transition is not an exactly correct name for this, not sure.
    fn transition(&mut self, from: f32, to: f32, duration: f32) {
        if !self.is_finished() {
            let position = self.get_value();
            let total_distance = (to - from).abs();
            let remaining_distance = (to - position).abs();
            let proportional_duration = duration * (remaining_distance / total_distance);
            self.start(position, to, proportional_duration);
            return;
        }

        if self.get_value() != to {
            self.start(from, to, duration);
        }
    }
}

struct Console {
    open_animation: Animation,

    command_editor: String,
    command_editor_state: uhi::TextState,
    command_editor_active: bool,

    history: String,
    history_state: uhi::TextState,
}

impl Console {
    const HEIGHT: f32 = 384.0;
    const ANIMATION_DURATION: f32 = 0.2;

    fn new() -> Self {
        Self {
            open_animation: Animation::default(),

            command_editor: "".to_string(),
            command_editor_state: uhi::TextState::default(),
            command_editor_active: false,

            history: "".to_string(),
            history_state: uhi::TextState::default(),
        }
    }

    fn is_open(&self) -> bool {
        self.open_animation.get_value() > -Self::HEIGHT
    }

    fn update_history<E: uhi::Externs>(
        &mut self,
        rect: uhi::Rect,
        ctx: &mut uhi::Context<E>,
        input: &input::State,
    ) {
        assert!(self.is_open());

        uhi::Text::new(self.history.as_str(), rect.shrink(&uhi::Vec2::splat(16.0)))
            .multiline()
            .selectable(&mut self.history_state)
            .draw(ctx, input);
    }

    fn update_command_editor<E: uhi::Externs>(
        &mut self,
        rect: uhi::Rect,
        ctx: &mut uhi::Context<E>,
        input: &input::State,
    ) {
        assert!(self.is_open());

        let key = uhi::Key::from_caller_location();

        ctx.interaction_state
            .maybe_set_hot_or_active(key, rect, input::CursorShape::Text, input);

        if self.open_animation.just_finished() {
            // if animation just finished -> activate the command editor.
            self.command_editor_active = true;
        } else if self.command_editor_active {
            // but then it needs to be deactivated *once*. future activations will be set by the
            // interaction state thingie.
            let any_button_pressed = input
                .pointer
                .buttons
                .any_just_pressed(input::PointerButton::all());
            let rect_contains_pointer =
                rect.contains(&uhi::Vec2::from(uhi::F64Vec2::from(input.pointer.position)));
            if any_button_pressed && !rect_contains_pointer {
                self.command_editor_active = false;
            }
        }

        let active = (self.command_editor_active || ctx.interaction_state.is_active(key))
            && self.open_animation.is_finished();

        // ----

        if active {
            let input::KeyboardState { ref scancodes, .. } = input.keyboard;

            if scancodes.just_pressed(input::Scancode::Enter) && !self.command_editor.is_empty() {
                self.history.push_str("> ");
                self.history.push_str(&self.command_editor);
                self.history.push('\n');

                self.command_editor.clear();
                self.command_editor_state.clear();

                // TODO: scroll history to end
            }
        }

        // ----

        // background
        ctx.draw_buffer.push_rect(uhi::RectShape::new(
            rect,
            uhi::Fill::new_with_color(uhi::Rgba8::from_u32(0xffffff0c)),
            uhi::Stroke {
                width: 1.0,
                color: if active {
                    uhi::Rgba8::from_u32(0x4393e7ff)
                } else {
                    uhi::Rgba8::from_u32(0xcccccc33)
                },
                alignment: uhi::StrokeAlignment::Inside,
            },
        ));

        let font_height = ctx
            .font_service
            .get_font_instance(ctx.default_font_handle(), ctx.default_font_size())
            .height();
        let py = (rect.height() - font_height) / 2.0;

        uhi::Text::new(
            &mut self.command_editor,
            rect.shrink(&uhi::Vec2::new(16.0, py)),
        )
        .with_key(key)
        .with_maybe_hot_or_active(ctx.interaction_state.is_hot(key), active)
        .singleline()
        .editable(&mut self.command_editor_state)
        .draw(ctx, input);
    }

    fn update<E: uhi::Externs>(
        &mut self,
        rect: uhi::Rect,
        ctx: &mut uhi::Context<E>,
        input: &input::State,
    ) {
        let input::KeyboardState { ref scancodes, .. } = input.keyboard;

        if scancodes.just_pressed(input::Scancode::Grave) && !self.is_open() {
            self.open_animation
                .transition(-Self::HEIGHT, 0.0, Self::ANIMATION_DURATION);
        }
        if scancodes.just_pressed(input::Scancode::Esc) && self.is_open() {
            self.open_animation
                .transition(0.0, -Self::HEIGHT, Self::ANIMATION_DURATION);
        }

        self.open_animation.maybe_step(ctx.dt());
        if !self.is_open() {
            return;
        }

        let container_rect = {
            let min = rect.min + uhi::Vec2::new(0.0, self.open_animation.get_value());
            uhi::Rect::new(min, min + uhi::Vec2::new(rect.max.x, Self::HEIGHT))
        };
        ctx.draw_buffer.push_rect(uhi::RectShape::new_with_fill(
            container_rect,
            uhi::Fill::new_with_color(uhi::Rgba8::from_u32(0x1f1f1fff)),
        ));

        let [history_container_rect, _gap, command_editor_container_rect] = uhi::vstack([
            uhi::Constraint::Fill(1.0),
            uhi::Constraint::Length(8.0),
            uhi::Constraint::Length(34.0),
        ])
        .split(container_rect);

        self.update_history(history_container_rect, ctx, input);
        self.update_command_editor(command_editor_container_rect, ctx, input);
    }
}

struct App {
    uhi_context: uhi::Context<UhiExterns>,
    uhi_renderer: uhi::GlRenderer,

    input_state: input::State,

    console: Console,
}

impl AppHandler for App {
    fn create(ctx: app::AppContext) -> Self {
        Self {
            uhi_context: uhi::Context::default(),
            uhi_renderer: uhi::GlRenderer::new(ctx.gl_api).expect("uhi gl renderer fucky wucky"),

            input_state: input::State::default(),

            console: Console::new(),
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

        unsafe { ctx.gl_api.clear_color(0.094, 0.094, 0.094, 1.0) };
        unsafe { ctx.gl_api.clear(gl::api::COLOR_BUFFER_BIT) };

        let physical_window_size = ctx.window.size();
        let scale_factor = ctx.window.scale_factor();
        let logical_window_rect = uhi::Rect::new(
            uhi::Vec2::ZERO,
            uhi::Vec2::from(uhi::U32Vec2::from(physical_window_size)) / scale_factor as f32,
        );

        uhi::Text::new(
            "press ` to open console",
            logical_window_rect.shrink(&uhi::Vec2::new(16.0, 16.0 * 1.0)),
        )
        .singleline()
        .draw(&mut self.uhi_context);

        self.console.update(
            logical_window_rect,
            &mut self.uhi_context,
            &self.input_state,
        );

        if let Some(cursor_shape) = self.uhi_context.interaction_state.take_cursor_shape() {
            ctx.window
                .set_cursor_shape(cursor_shape)
                // TODO: proper error handling
                .expect("could not set cursor shape");
        }

        if self.uhi_context.clipboard_state.is_awaiting_read() {
            // TODO: figure out maybe how to incorporate reusable clipboard-read buffer into uhi
            // context or something?
            let mut buf = vec![];
            let payload = ctx
                .window
                .read_clipboard(window::MIME_TYPE_TEXT, &mut buf)
                .and_then(|_| String::from_utf8(buf).context("invalid text"));
            self.uhi_context.clipboard_state.fulfill_read(payload);
        }

        if let Some(text) = self.uhi_context.clipboard_state.take_write() {
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
