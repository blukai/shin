use app::AppHandler;
use gl::api::Apier as _;
use window::{Event, WindowAttrs};

struct App;

impl AppHandler for App {
    fn create(_ctx: app::AppContext) -> Self {
        Self
    }

    fn handle_event(&mut self, _ctx: app::AppContext, _event: Event) {}

    fn update(&mut self, ctx: app::AppContext) {
        unsafe { ctx.gl_api.clear_color(1.0, 0.0, 0.0, 1.0) };
        unsafe { ctx.gl_api.clear(gl::api::COLOR_BUFFER_BIT) };
    }
}

fn main() {
    app::run::<App>(WindowAttrs::default());
}
