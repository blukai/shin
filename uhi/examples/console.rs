use app::AppHandler;
use gl::api::Apier as _;
use window::{Event, WindowAttrs, WindowEvent};

struct UhiExterns;

impl uhi::Externs for UhiExterns {
    type TextureHandle = <uhi::GlRenderer as uhi::Renderer>::TextureHandle;
}

#[derive(Default)]
struct Animation {
    from: f32,
    to: f32,
    duration: f32,
    elapsed: f32,
}

impl Animation {
    fn start(&mut self, from: f32, to: f32, duration: f32) {
        self.from = from;
        self.to = to;
        self.duration = duration;
        self.elapsed = 0.0;
    }

    fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    fn step(&mut self, dt: f32) {
        self.elapsed += dt;
    }

    fn get_value(&mut self) -> f32 {
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
        } else if self.get_value() != to {
            self.start(from, to, duration);
        }
    }
}

struct Console {
    y: f32,
    open_animation: Animation,

    command_editor: String,
    command_editor_selection: uhi::TextSelection,
    command_editor_active: bool,

    history: String,
    history_selection: uhi::TextSelection,
}

impl Console {
    const HEIGHT: f32 = 384.0;
    const ANIMATION_DURATION: f32 = 0.2;

    fn new() -> Self {
        Self {
            y: -Self::HEIGHT,
            open_animation: Animation::default(),

            command_editor: "".to_string(),
            command_editor_selection: uhi::TextSelection::default(),
            command_editor_active: false,

            history: "".to_string(),
            history_selection: uhi::TextSelection::default(),
        }
    }

    fn is_open(&self) -> bool {
        self.y > -Self::HEIGHT
    }

    fn update_history<E: uhi::Externs>(
        &mut self,
        rect: uhi::Rect,
        ctx: &mut uhi::Context<E>,
        input: &input::State,
    ) {
        let key = uhi::Key::from_location();

        let history_became_active = ctx
            .interaction_state
            .maybe_set_hot_or_active(key, rect, input);
        if history_became_active {
            self.command_editor_active = false;
        }

        uhi::Text::new(self.history.as_str(), rect.shrink(&uhi::Vec2::splat(16.0)))
            .multiline()
            .selectable(&mut self.history_selection)
            .with_hot(ctx.interaction_state.is_hot(key))
            .with_active(ctx.interaction_state.is_active(key))
            .update_if(|t| t.is_active(), ctx, input)
            .draw(ctx);
    }

    fn update_command_editor<E: uhi::Externs>(
        &mut self,
        rect: uhi::Rect,
        ctx: &mut uhi::Context<E>,
        input: &input::State,
    ) {
        let key = uhi::Key::from_location();

        let command_editor_became_active = ctx
            .interaction_state
            .maybe_set_hot_or_active(key, rect, input);
        if command_editor_became_active {
            self.command_editor_active = true;
        }
        let command_editor_active = self.command_editor_active && self.open_animation.is_finished();

        let font_height = ctx
            .font_service
            .get_font_instance(ctx.default_font_handle, ctx.default_font_size)
            .height();
        let py = (rect.height() - font_height) / 2.0;

        // TODO: vertically center text
        ctx.draw_buffer.push_rect(uhi::RectShape::with_fill(
            rect,
            uhi::Fill::with_color(uhi::Rgba8::RED),
        ));
        uhi::Text::new(
            &mut self.command_editor,
            rect.shrink(&uhi::Vec2::new(16.0, py)),
        )
        .singleline()
        .editable(&mut self.command_editor_selection)
        .with_hot(ctx.interaction_state.is_hot(key))
        .with_active(command_editor_active)
        .update_if(|t| t.is_active(), ctx, input)
        .draw(ctx);

        if command_editor_active {
            let input::KeyboardState { ref scancodes, .. } = input.keyboard;

            if scancodes.just_pressed(input::Scancode::Enter) && !self.command_editor.is_empty() {
                self.history.push_str("> ");
                self.history.push_str(&self.command_editor);
                self.history.push('\n');

                self.command_editor.clear();
                self.command_editor_selection.clear();

                // TODO: scroll history to end
            }
        }
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
            self.command_editor_active = true;
        }
        if scancodes.just_pressed(input::Scancode::Esc) && self.is_open() {
            self.open_animation
                .transition(0.0, -Self::HEIGHT, Self::ANIMATION_DURATION);
            self.command_editor_active = false;
        }

        if !self.open_animation.is_finished() {
            self.open_animation.step(ctx.dt());
            self.y = self.open_animation.get_value();
        }

        if !self.is_open() {
            return;
        }

        let container_rect = {
            let min = rect.min + uhi::Vec2::new(0.0, self.y);
            uhi::Rect::new(min, min + uhi::Vec2::new(rect.max.x, Self::HEIGHT))
        };
        ctx.draw_buffer.push_rect(uhi::RectShape::with_fill(
            container_rect,
            uhi::Fill::with_color(uhi::Rgba8::GRAY),
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

        unsafe { ctx.gl_api.clear_color(0.1, 0.2, 0.2, 1.0) };
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
