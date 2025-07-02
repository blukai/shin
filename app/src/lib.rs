use window::{Event, Window};

pub struct AppContext<'a> {
    pub window: &'a mut dyn Window,
    pub gl_api: &'a mut gl::api::Api,
}

pub trait AppHandler {
    fn create(ctx: AppContext) -> Self;
    fn handle_event(&mut self, ctx: AppContext, event: Event);
    fn update(&mut self, ctx: AppContext);
}

#[cfg(unix)]
mod app_native;
#[cfg(unix)]
pub use app_native::run;

#[cfg(target_family = "wasm")]
mod app_web;
#[cfg(target_family = "wasm")]
pub use app_web::run;
