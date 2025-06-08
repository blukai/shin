use app::AppHandler;
use gpu::gl::{self, GlContext};
use window::{Event, WindowAttrs};

struct App;

impl AppHandler for App {
    fn create(_ctx: app::AppContext) -> Self {
        Self
    }

    fn handle_event(&mut self, _ctx: app::AppContext, _event: Event) {}

    fn update(&mut self, ctx: app::AppContext) {
        unsafe { ctx.gl.clear_color(1.0, 0.0, 0.0, 1.0) };
        unsafe { ctx.gl.clear(gl::COLOR_BUFFER_BIT) };
    }
}

fn main() {
    app::run::<App>(WindowAttrs::default());
}
