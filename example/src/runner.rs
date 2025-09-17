use window::{Event, Window};

#[cfg(not(target_family = "wasm"))]
#[path = "runner_native.rs"]
mod runner_native;

#[cfg(target_family = "wasm")]
#[path = "runner_web.rs"]
mod runner_web;

#[cfg(not(target_family = "wasm"))]
pub use runner_native::run;

#[cfg(target_family = "wasm")]
pub use runner_web::run;

pub struct Context<'a> {
    pub window: &'a mut dyn Window,
    pub gl_api: &'a mut gl::Api,
}

pub trait Handler {
    fn create(ctx: Context) -> Self;
    fn iterate(&mut self, ctx: Context, events: impl Iterator<Item = Event>);
}
