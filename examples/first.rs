use app::AppHandler;
use gl::Apier as _;
use window::{Event, WindowAttrs};

struct App;

impl AppHandler for App {
    fn create(_ctx: app::AppContext) -> Self {
        Self
    }

    fn iterate(&mut self, ctx: app::AppContext, _events: impl Iterator<Item = Event>) {
        unsafe { ctx.gl_api.clear_color(1.0, 0.0, 0.0, 1.0) };
        unsafe { ctx.gl_api.clear(gl::COLOR_BUFFER_BIT) };
    }
}

fn main() {
    app::run::<App>(WindowAttrs::default());
}
