use anyhow::Context as _;
use app::AppHandler;
use gl::api::Apier as _;
use window::{Event, WindowAttrs};

struct GuiExterns;

impl gui::Externs for GuiExterns {
    type TextureHandle = <gui::GlRenderer as gui::Renderer>::TextureHandle;
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
    command_editor_state: gui::TextState,
    command_editor_active: bool,

    history: String,
    history_state: gui::TextState,
}

impl Console {
    const HEIGHT: f32 = 384.0;
    const ANIMATION_DURATION: f32 = 0.2;

    fn new() -> Self {
        Self {
            open_animation: Animation::default(),

            command_editor: "".to_string(),
            command_editor_state: gui::TextState::default(),
            command_editor_active: false,

            history: "".to_string(),
            history_state: gui::TextState::default(),
        }
    }

    fn is_open(&self) -> bool {
        self.open_animation.get_value() > -Self::HEIGHT
    }

    fn update_history<E: gui::Externs>(
        &mut self,
        rect: gui::Rect,
        ctx: &mut gui::Context<E>,
        vpt: &mut gui::Viewport<E>,
        input: &input::State,
    ) {
        assert!(self.is_open());

        gui::Text::new_selectable(
            self.history.as_str(),
            rect.inflate(-gui::Vec2::splat(16.0)),
            &mut self.history_state,
        )
        .multiline()
        .draw(ctx, vpt, input);
    }

    fn update_command_editor<E: gui::Externs>(
        &mut self,
        rect: gui::Rect,
        ctx: &mut gui::Context<E>,
        vpt: &mut gui::Viewport<E>,
        input: &input::State,
    ) {
        assert!(self.is_open());

        let key = gui::Key::from_caller_location();

        ctx.interaction_state.maybe_interact(
            gui::InteractionRequest::new(key, rect).with_cursor_shape(input::CursorShape::Text),
            input,
        );

        if self.open_animation.just_finished() {
            // if animation just finished -> activate the command editor.
            self.command_editor_active = true;
        } else if self.command_editor_active {
            // but then it needs to be deactivated *once*. future activations will be set by the
            // interaction state thingie.
            let any_button_pressed = input.pointer.buttons.any_just_pressed(input::Button::all());
            let rect_contains_pointer =
                rect.contains(gui::Vec2::from(gui::F64Vec2::from(input.pointer.position)));
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
        vpt.draw_buffer.push_rect(gui::RectShape::new(
            rect,
            gui::Fill::new_with_color(gui::Rgba8::from_u32(0xffffff0c)),
            gui::Stroke {
                width: 1.0,
                color: if active {
                    gui::Rgba8::from_u32(0x4393e7ff)
                } else {
                    gui::Rgba8::from_u32(0xcccccc33)
                },
                alignment: gui::StrokeAlignment::Inside,
            },
        ));

        let font_height = ctx
            .font_service
            .get_or_create_font_instance(
                ctx.appearance.font_handle,
                ctx.appearance.font_size,
                vpt.scale_factor,
            )
            .height();
        let py = (rect.height() - font_height) / 2.0;

        gui::Text::new_editable(
            &mut self.command_editor,
            rect.inflate(-gui::Vec2::new(16.0, py)),
            &mut self.command_editor_state,
        )
        .with_key(key)
        .with_maybe_hot_or_active(ctx.interaction_state.is_hot(key), active)
        .singleline()
        .draw(ctx, vpt, input);
    }

    fn update<E: gui::Externs>(
        &mut self,
        rect: gui::Rect,
        ctx: &mut gui::Context<E>,
        vpt: &mut gui::Viewport<E>,
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

        self.open_animation.maybe_step(vpt.dt());
        if !self.is_open() {
            return;
        }

        let container_rect = {
            let min = rect.min + gui::Vec2::new(0.0, self.open_animation.get_value());
            gui::Rect::new(min, min + gui::Vec2::new(rect.max.x, Self::HEIGHT))
        };
        vpt.draw_buffer.push_rect(gui::RectShape::new_with_fill(
            container_rect,
            gui::Fill::new_with_color(gui::Rgba8::from_u32(0x1f1f1fff)),
        ));

        let [history_container_rect, _gap, command_editor_container_rect] = gui::vstack([
            gui::Constraint::Fill(1.0),
            gui::Constraint::Length(8.0),
            gui::Constraint::Length(34.0),
        ])
        .split(container_rect);

        self.update_history(history_container_rect, ctx, vpt, input);
        self.update_command_editor(command_editor_container_rect, ctx, vpt, input);
    }
}

struct App {
    gui_context: gui::Context<GuiExterns>,
    gui_viewport: gui::Viewport<GuiExterns>,
    gui_renderer: gui::GlRenderer,

    input_state: input::State,

    console: Console,
}

impl AppHandler for App {
    fn create(ctx: app::AppContext) -> Self {
        Self {
            gui_context: gui::Context::default(),
            gui_viewport: gui::Viewport::default(),
            gui_renderer: gui::GlRenderer::new(ctx.gl_api).expect("gui gl renderer fucky wucky"),

            input_state: input::State::default(),

            console: Console::new(),
        }
    }

    fn iterate(&mut self, ctx: app::AppContext, events: impl Iterator<Item = Event>) {
        let physical_size = gui::Vec2::from(gui::U32Vec2::from(ctx.window.physical_size()));
        let scale_factor = ctx.window.scale_factor() as f32;

        self.input_state
            .begin_iteration(events.filter_map(|event| match event {
                Event::Window(_) => None,
                Event::Pointer(pointer_event) => Some(input::Event::Pointer(pointer_event)),
                Event::Keyboard(keyboard_event) => Some(input::Event::Keyboard(keyboard_event)),
            }));
        self.gui_context.begin_iteration(&self.input_state);
        self.gui_viewport.begin_frame(physical_size, scale_factor);

        // ----

        unsafe { ctx.gl_api.clear_color(0.094, 0.094, 0.094, 1.0) };
        unsafe { ctx.gl_api.clear(gl::api::COLOR_BUFFER_BIT) };

        let logical_size = physical_size / scale_factor;
        let logical_rect = gui::Rect::new(gui::Vec2::ZERO, logical_size);

        gui::Text::new_non_interactive(
            "press ` to open console",
            logical_rect.inflate(-gui::Vec2::new(16.0, 16.0 * 1.0)),
        )
        .singleline()
        .draw(&mut self.gui_context, &mut self.gui_viewport);

        self.console.update(
            logical_rect,
            &mut self.gui_context,
            &mut self.gui_viewport,
            &self.input_state,
        );

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
