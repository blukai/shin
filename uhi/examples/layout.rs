use app::AppHandler;
use gl::api::Apier as _;
use window::{Event, WindowAttrs, WindowEvent};

const FONT: &[u8] = include_bytes!("../../fixtures/JetBrainsMono-Regular.ttf");

struct UhiExterns;

impl uhi::Externs for UhiExterns {
    type TextureHandle = <uhi::GlRenderer as uhi::Renderer>::TextureHandle;
}

fn draw<E: uhi::Externs>(ctx: &mut uhi::Context<E>, font_handle: uhi::FontHandle, area: uhi::Rect) {
    use uhi::*;

    // Tableau I, by Piet Mondriaan
    // https://en.wikipedia.org/wiki/File:Tableau_I,_by_Piet_Mondriaan.jpg

    // TODO: figure out a nicer way to layout and draw stuff. with no heap allocations for the
    // layout!

    const SIZE: Vec2 = Vec2::new(1130.0, 1200.0);

    const TOP_HEIGHT: f32 = 640.0;
    const GAP: Constraint = uhi::Constraint::Length(20.0);
    const BOTTOM_HEIGHT: f32 = 540.0;

    let [top, gap, bottom] = vstack([
        Constraint::Percentage(TOP_HEIGHT / SIZE.y),
        GAP,
        Constraint::Percentage(BOTTOM_HEIGHT / SIZE.y),
    ])
    .split(area.clone());

    // top
    {
        ctx.draw_buffer.push_rect(RectShape::with_fill(
            top.clone(),
            Fill::with_color(Rgba8::WHITE),
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
                ctx.draw_buffer
                    .push_rect(RectShape::with_fill(top, Fill::with_color(Rgba8::RED)));
                ctx.draw_buffer
                    .push_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));
                ctx.draw_buffer
                    .push_rect(RectShape::with_fill(bottom, Fill::with_color(Rgba8::WHITE)));
            }

            ctx.draw_buffer
                .push_rect(RectShape::with_fill(lgap, Fill::with_color(Rgba8::BLACK)));
            ctx.draw_buffer
                .push_rect(RectShape::with_fill(mid, Fill::with_color(Rgba8::WHITE)));
            ctx.draw_buffer
                .push_rect(RectShape::with_fill(rgap, Fill::with_color(Rgba8::BLACK)));

            {
                let [top, tgap, mid, bgap, bottom] = vstack([
                    Constraint::Percentage(80.0 / TOP_HEIGHT),
                    GAP,
                    Constraint::Fill(1.0),
                    GAP,
                    Constraint::Percentage(130.0 / TOP_HEIGHT),
                ])
                .split(right);
                ctx.draw_buffer
                    .push_rect(RectShape::with_fill(top, Fill::with_color(Rgba8::BLACK)));
                ctx.draw_buffer
                    .push_rect(RectShape::with_fill(tgap, Fill::with_color(Rgba8::BLACK)));
                ctx.draw_buffer
                    .push_rect(RectShape::with_fill(mid, Fill::with_color(Rgba8::WHITE)));
                ctx.draw_buffer
                    .push_rect(RectShape::with_fill(bgap, Fill::with_color(Rgba8::BLACK)));
                ctx.draw_buffer
                    .push_rect(RectShape::with_fill(bottom, Fill::with_color(Rgba8::WHITE)));
            }
        }
    }

    ctx.draw_buffer
        .push_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));

    // bottom
    {
        ctx.draw_buffer.push_rect(RectShape::with_fill(
            bottom.clone(),
            Fill::with_color(Rgba8::WHITE),
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
                lmgap = gap.clone();
                ctx.draw_buffer
                    .push_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));

                {
                    let [top, gap, _] = vstack([
                        Constraint::Percentage(300.0 / BOTTOM_HEIGHT),
                        GAP,
                        Constraint::Fill(1.0),
                    ])
                    .split(right);
                    ctx.draw_buffer
                        .push_rect(RectShape::with_fill(top, Fill::with_color(Rgba8::BLUE)));
                    ctx.draw_buffer
                        .push_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));
                }
            }

            ctx.draw_buffer.push_rect(RectShape::with_fill(
                lgap.clone(),
                Fill::with_color(Rgba8::BLACK),
            ));
            ctx.draw_buffer.push_rect(RectShape::with_fill(
                rgap.clone(),
                Fill::with_color(Rgba8::BLACK),
            ));
            ctx.draw_buffer
                .push_rect(RectShape::with_fill(right, Fill::with_color(Rgba8::ORANGE)));

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
                    Vec2::new(max_x, area.max.y),
                ));
                ctx.draw_buffer
                    .push_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));

                {
                    let [left, gap, right] = hstack([
                        Constraint::Fill(1.0),
                        GAP,
                        Constraint::Percentage(100.0 / SIZE.x),
                    ])
                    .split(bottom);
                    ctx.draw_buffer
                        .push_rect(RectShape::with_fill(left, Fill::with_color(Rgba8::WHITE)));
                    ctx.draw_buffer
                        .push_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));
                    ctx.draw_buffer
                        .push_rect(RectShape::with_fill(right, Fill::with_color(Rgba8::BLACK)));
                }
            }
        }
    }

    uhi::Text::new(
        "Tableau I, by Piet Mondriaan",
        font_handle,
        14.0,
        area.shrink(&Vec2::splat(24.0)),
    )
    .with_palette(uhi::TextPalette::default().with_fg(uhi::Rgba8::FUCHSIA))
    .singleline()
    .draw(ctx);
}

struct App {
    uhi_context: uhi::Context<UhiExterns>,
    uhi_renderer: uhi::GlRenderer,

    font_handle: uhi::FontHandle,
    input_state: input::State,
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
        self.uhi_context.begin_frame();

        // ----

        unsafe { ctx.gl_api.clear_color(0.0, 0.0, 0.3, 1.0) };
        unsafe { ctx.gl_api.clear(gl::api::COLOR_BUFFER_BIT) };

        let physical_window_size = ctx.window.size();
        let scale_factor = ctx.window.scale_factor();
        let logical_window_rect = uhi::Rect::new(
            uhi::Vec2::ZERO,
            uhi::Vec2::from(uhi::U32Vec2::from(physical_window_size)) / scale_factor as f32,
        );

        draw(&mut self.uhi_context, self.font_handle, logical_window_rect);

        self.uhi_renderer
            .render(
                &mut self.uhi_context,
                ctx.gl_api,
                physical_window_size,
                scale_factor,
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
