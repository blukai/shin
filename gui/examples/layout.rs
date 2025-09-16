use app::AppHandler;
use gl::Apier as _;
use window::{Event, WindowAttrs};

struct GuiExterns;

impl gui::Externs for GuiExterns {
    type TextureHandle = <gui::GlRenderer as gui::Renderer>::TextureHandle;
}

fn draw<E: gui::Externs>(ctx: &mut gui::Context<E>, vpt: &mut gui::Viewport<E>) {
    use gui::*;

    // Tableau I, by Piet Mondriaan
    // https://en.wikipedia.org/wiki/File:Tableau_I,_by_Piet_Mondriaan.jpg

    // TODO: figure out a nicer way to layout and draw stuff. with no heap allocations for the
    // layout!

    const SIZE: Vec2 = Vec2::new(1130.0, 1200.0);

    const TOP_HEIGHT: f32 = 640.0;
    const GAP: Constraint = gui::Constraint::Length(20.0);
    const BOTTOM_HEIGHT: f32 = 540.0;

    let viewport_logical_size = vpt.physical_size / vpt.scale_factor;
    let viewport_area = Rect::new(Vec2::ZERO, viewport_logical_size);

    let [top, gap, bottom] = vstack([
        Constraint::Percentage(TOP_HEIGHT / SIZE.y),
        GAP,
        Constraint::Percentage(BOTTOM_HEIGHT / SIZE.y),
    ])
    .split(viewport_area);

    // top
    {
        vpt.draw_buffer.push_rect(RectShape::new_with_fill(
            top,
            Fill::new_with_color(Rgba::WHITE),
        ));

        {
            let [left, lgap, mid, rgap, right] = hstack([
                Constraint::Percentage(500.0 / SIZE.x),
                GAP,
                Constraint::Fill(1.0),
                GAP,
                Constraint::Percentage(170.0 / SIZE.x),
            ])
            .split(top);

            {
                let [top, gap, bottom] = vstack([
                    Constraint::Percentage(10.0 / TOP_HEIGHT),
                    GAP,
                    Constraint::Fill(1.0),
                ])
                .split(left);
                vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                    top,
                    Fill::new_with_color(Rgba::RED),
                ));
                vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                    gap,
                    Fill::new_with_color(Rgba::BLACK),
                ));
                vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                    bottom,
                    Fill::new_with_color(Rgba::WHITE),
                ));
            }

            vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                lgap,
                Fill::new_with_color(Rgba::BLACK),
            ));
            vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                mid,
                Fill::new_with_color(Rgba::WHITE),
            ));
            vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                rgap,
                Fill::new_with_color(Rgba::BLACK),
            ));

            {
                let [top, tgap, mid, bgap, bottom] = vstack([
                    Constraint::Percentage(80.0 / TOP_HEIGHT),
                    GAP,
                    Constraint::Fill(1.0),
                    GAP,
                    Constraint::Percentage(130.0 / TOP_HEIGHT),
                ])
                .split(right);
                vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                    top,
                    Fill::new_with_color(Rgba::BLACK),
                ));
                vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                    tgap,
                    Fill::new_with_color(Rgba::BLACK),
                ));
                vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                    mid,
                    Fill::new_with_color(Rgba::WHITE),
                ));
                vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                    bgap,
                    Fill::new_with_color(Rgba::BLACK),
                ));
                vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                    bottom,
                    Fill::new_with_color(Rgba::WHITE),
                ));
            }
        }
    }

    vpt.draw_buffer.push_rect(RectShape::new_with_fill(
        gap,
        Fill::new_with_color(Rgba::BLACK),
    ));

    // bottom
    {
        vpt.draw_buffer.push_rect(RectShape::new_with_fill(
            bottom,
            Fill::new_with_color(Rgba::WHITE),
        ));

        {
            let [left, lgap, _, rgap, right] = hstack([
                Constraint::Percentage(500.0 / SIZE.x),
                GAP,
                Constraint::Fill(1.0),
                GAP,
                Constraint::Percentage(170.0 / SIZE.x),
            ])
            .split(bottom);

            let lmgap;
            {
                let [_, gap, right] = hstack([
                    Constraint::Percentage(100.0 / SIZE.x),
                    GAP,
                    Constraint::Fill(1.0),
                ])
                .split(left);
                lmgap = gap;
                vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                    gap,
                    Fill::new_with_color(Rgba::BLACK),
                ));

                {
                    let [top, gap, _] = vstack([
                        Constraint::Percentage(300.0 / BOTTOM_HEIGHT),
                        GAP,
                        Constraint::Fill(1.0),
                    ])
                    .split(right);
                    vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                        top,
                        Fill::new_with_color(Rgba::BLUE),
                    ));
                    vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                        gap,
                        Fill::new_with_color(Rgba::BLACK),
                    ));
                }
            }

            vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                lgap,
                Fill::new_with_color(Rgba::BLACK),
            ));
            vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                rgap,
                Fill::new_with_color(Rgba::BLACK),
            ));
            vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                right,
                Fill::new_with_color(Rgba::ORANGE),
            ));

            // bottom-mid section [white .......... black ..]
            {
                let min_x = lmgap.max.x;
                let max_x = rgap.min.x;
                let [_, gap, bottom] = vstack([
                    Constraint::Fill(1.0),
                    GAP,
                    Constraint::Percentage(30.0 / SIZE.y),
                ])
                .split(Rect::new(
                    Vec2::new(min_x, 0.0),
                    Vec2::new(max_x, viewport_area.max.y),
                ));
                vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                    gap,
                    Fill::new_with_color(Rgba::BLACK),
                ));

                {
                    let [left, gap, right] = hstack([
                        Constraint::Fill(1.0),
                        GAP,
                        Constraint::Percentage(100.0 / SIZE.x),
                    ])
                    .split(bottom);
                    vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                        left,
                        Fill::new_with_color(Rgba::WHITE),
                    ));
                    vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                        gap,
                        Fill::new_with_color(Rgba::BLACK),
                    ));
                    vpt.draw_buffer.push_rect(RectShape::new_with_fill(
                        right,
                        Fill::new_with_color(Rgba::BLACK),
                    ));
                }
            }
        }
    }

    gui::Text::new_non_interactive(
        "Tableau I, by Piet Mondriaan",
        viewport_area.inflate(-Vec2::splat(24.0)),
    )
    .with_appearance(
        gui::TextAppearance::from_appearance(&ctx.appearance).with_fg(gui::Rgba::FUCHSIA),
    )
    .singleline()
    .draw(ctx, vpt);
}

struct App {
    gui_context: gui::Context<GuiExterns>,
    gui_viewport: gui::Viewport<GuiExterns>,
    gui_renderer: gui::GlRenderer,

    input_state: input::State,
}

impl AppHandler for App {
    fn create(ctx: app::AppContext) -> Self {
        Self {
            gui_context: gui::Context::default(),
            gui_viewport: gui::Viewport::default(),
            gui_renderer: gui::GlRenderer::new(ctx.gl_api).expect("gui gl renderer fucky wucky"),

            input_state: input::State::default(),
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

        unsafe { ctx.gl_api.clear_color(0.0, 0.0, 0.3, 1.0) };
        unsafe { ctx.gl_api.clear(gl::COLOR_BUFFER_BIT) };

        draw(&mut self.gui_context, &mut self.gui_viewport);

        self.gui_context.interaction_state.take_cursor_shape();

        self.gui_renderer
            .render(&mut self.gui_context, &mut self.gui_viewport, ctx.gl_api)
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
