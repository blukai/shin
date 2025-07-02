use app::AppHandler;
use gl::api::Apier as _;
use glam::Vec2;
use window::{Event, WindowAttrs, WindowEvent};

const FONT: &[u8] = include_bytes!("../../fixtures/JetBrainsMono-Regular.ttf");

struct UhiExterns;

impl uhi::Externs for UhiExterns {
    type TextureHandle = <uhi::GlRenderer as uhi::Renderer>::TextureHandle;
}

// Tableau I, by Piet Mondriaan
// https://en.wikipedia.org/wiki/File:Tableau_I,_by_Piet_Mondriaan.jpg
fn draw_mondriaan<E: uhi::Externs>(
    uhi: &mut uhi::Context<E>,
    font_handle: uhi::FontHandle,
    area: uhi::Rect,
) {
    use uhi::*;

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
        uhi.draw_rect(RectShape::with_fill(
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
                uhi.draw_rect(RectShape::with_fill(top, Fill::with_color(Rgba8::RED)));
                uhi.draw_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));
                uhi.draw_rect(RectShape::with_fill(bottom, Fill::with_color(Rgba8::WHITE)));
            }

            uhi.draw_rect(RectShape::with_fill(lgap, Fill::with_color(Rgba8::BLACK)));
            uhi.draw_rect(RectShape::with_fill(mid, Fill::with_color(Rgba8::WHITE)));
            uhi.draw_rect(RectShape::with_fill(rgap, Fill::with_color(Rgba8::BLACK)));

            {
                let [top, tgap, mid, bgap, bottom] = vstack([
                    Constraint::Percentage(80.0 / TOP_HEIGHT),
                    GAP,
                    Constraint::Fill(1.0),
                    GAP,
                    Constraint::Percentage(130.0 / TOP_HEIGHT),
                ])
                .split(right);
                uhi.draw_rect(RectShape::with_fill(top, Fill::with_color(Rgba8::BLACK)));
                uhi.draw_rect(RectShape::with_fill(tgap, Fill::with_color(Rgba8::BLACK)));
                uhi.draw_rect(RectShape::with_fill(mid, Fill::with_color(Rgba8::WHITE)));
                uhi.draw_rect(RectShape::with_fill(bgap, Fill::with_color(Rgba8::BLACK)));
                uhi.draw_rect(RectShape::with_fill(bottom, Fill::with_color(Rgba8::WHITE)));
            }
        }
    }

    uhi.draw_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));

    // bottom
    {
        uhi.draw_rect(RectShape::with_fill(
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
                uhi.draw_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));

                {
                    let [top, gap, _] = vstack([
                        Constraint::Percentage(300.0 / BOTTOM_HEIGHT),
                        GAP,
                        Constraint::Fill(1.0),
                    ])
                    .split(right);
                    uhi.draw_rect(RectShape::with_fill(top, Fill::with_color(Rgba8::BLUE)));
                    uhi.draw_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));
                }
            }

            uhi.draw_rect(RectShape::with_fill(
                lgap.clone(),
                Fill::with_color(Rgba8::BLACK),
            ));
            uhi.draw_rect(RectShape::with_fill(
                rgap.clone(),
                Fill::with_color(Rgba8::BLACK),
            ));
            uhi.draw_rect(RectShape::with_fill(right, Fill::with_color(Rgba8::ORANGE)));

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
                uhi.draw_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));

                {
                    let [left, gap, right] = hstack([
                        Constraint::Fill(1.0),
                        GAP,
                        Constraint::Percentage(100.0 / SIZE.x),
                    ])
                    .split(bottom);
                    uhi.draw_rect(RectShape::with_fill(left, Fill::with_color(Rgba8::WHITE)));
                    uhi.draw_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));
                    uhi.draw_rect(RectShape::with_fill(right, Fill::with_color(Rgba8::BLACK)));
                }
            }
        }
    }

    let text = "Tableau I, by Piet Mondriaan";
    let font_size = 14.0;

    let text_width =
        uhi.font_service
            .get_text_width(text, font_handle, font_size, &mut uhi.texture_service);
    let font_line_height = uhi
        .font_service
        .get_font_line_height(font_handle, font_size);
    let text_size = Vec2::new(text_width, font_line_height);
    let text_position = area.size() - Vec2::splat(24.0) - text_size;
    uhi.draw_rect(RectShape::with_fill(
        Rect::new(text_position, text_position + text_size),
        Fill::with_color(Rgba8::new(128, 128, 128, 128)),
    ));
    uhi.draw_text(text, font_handle, font_size, text_position, Rgba8::WHITE);
}

struct App {
    uhi_context: uhi::Context<UhiExterns>,
    uhi_font_handle: uhi::FontHandle,
    uhi_renderer: uhi::GlRenderer,

    input_state: input::State,
}

impl AppHandler for App {
    fn create(ctx: app::AppContext) -> Self {
        let mut uhi_context = uhi::Context::default();
        let uhi_font_handle = uhi_context
            .font_service
            .register_font_slice(FONT)
            .expect("invalid font");
        let uhi_renderer = uhi::GlRenderer::new(ctx.gl_api).expect("uhi gl renderer fucky wucky");

        Self {
            uhi_context,
            uhi_font_handle,
            uhi_renderer,

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
                self.input_state.handle_pointer_event(ev);
            }
            Event::Keyboard(ev) => {
                self.input_state.handle_keyboard_event(ev);
            }
            _ => {}
        }
    }

    fn update(&mut self, ctx: app::AppContext) {
        let window_size = ctx.window.size();

        unsafe { ctx.gl_api.clear_color(0.0, 0.0, 0.3, 1.0) };
        unsafe { ctx.gl_api.clear(gl::api::COLOR_BUFFER_BIT) };

        draw_mondriaan(
            &mut self.uhi_context,
            self.uhi_font_handle,
            uhi::Rect::new(
                Vec2::ZERO,
                Vec2::new(window_size.0 as f32, window_size.1 as f32),
            ),
        );
        // TextEdit::new(UhiId::Pep, &mut "kek".to_string()).draw(uhi, font_handle);

        self.uhi_renderer
            .render(&mut self.uhi_context, ctx.gl_api, window_size)
            .expect("uhi renderer fucky wucky");
        self.uhi_context.clear_draw_buffer();

        // TODO: make input state clearing better. find a better place for it? make less manual?
        // idk. make it better somehow.
        self.input_state.pointer.buttons.clear();
        self.input_state.keyboard.scancodes.clear();
        self.input_state.keyboard.keycodes.clear();
    }
}

fn main() {
    app::run::<App>(WindowAttrs::default());
}
