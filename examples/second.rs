use std::ffi::{CString, c_void};
use std::panic;

use anyhow::Context as _;
use graphics::gl::{self, Contexter};
use window::{Window, WindowAttrs, WindowEvent};

fn panic_hook(info: &panic::PanicHookInfo) {
    let msg = CString::new(info.to_string()).expect("invalid panic info");
    unsafe { window::js_sys::panic(msg.as_ptr()) };
}

struct ConsoleLogger;

impl log::Log for ConsoleLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        let msg = CString::new(format!(
            "{level:<5} {file}:{line} > {text}",
            level = record.level(),
            file = record.file().unwrap_or_else(|| record.target()),
            line = record
                .line()
                .map_or_else(|| "??".to_string(), |line| line.to_string()),
            text = record.args(),
        ))
        .expect("invalid console log message");
        unsafe { window::js_sys::console_log(msg.as_ptr()) };
    }

    fn flush(&self) {}
}

impl ConsoleLogger {
    fn init() {
        log::set_logger(&ConsoleLogger).expect("could not set logger");
        log::set_max_level(log::LevelFilter::Trace);
    }
}

struct Context {
    window: Box<dyn Window>,
    gl_context: Option<gl::Context>,
    i: usize,
}

impl Context {
    fn new() -> anyhow::Result<Self> {
        let window =
            window::create_window(WindowAttrs::default()).context("could not create window")?;
        Ok(Context {
            window,
            gl_context: None,
            i: 0,
        })
    }

    fn iterate(&mut self) -> anyhow::Result<()> {
        log::info!("iterate {}", self.i);

        while let Some(event) = self.window.pop_event() {
            log::debug!("event: {event:?}");

            match event {
                WindowEvent::Configure { logical_size } => {
                    if self.gl_context.is_none() {
                        self.gl_context = Some(gl::Context::from_window_handle(
                            self.window.window_handle()?,
                        ));
                    }
                }
                _ => {}
            }
        }

        if let Some(ctx) = self.gl_context.as_ref() {
            unsafe {
                ctx.clear_color(1.0, 0.0, 0.0, 1.0);
                ctx.clear(gl::COLOR_BUFFER_BIT);
            }
        }

        self.i += 1;
        Ok(())
    }
}

fn main() {
    panic::set_hook(Box::new(panic_hook));
    ConsoleLogger::init();

    // TODO: figure out wasm-side lifetime of the entire thing
    let ctx = Box::new(Context::new().expect("could not create context"));
    let mut ctx = std::mem::ManuallyDrop::new(ctx);

    unsafe extern "C" fn iterate(ctx: *mut c_void) -> bool {
        let ctx = unsafe { &mut *(ctx as *mut Context) };
        ctx.iterate().expect("iteration failure");
        return true;
    }
    let ctx_ptr = ctx.as_mut() as *mut Context as *mut c_void;
    unsafe { window::js_sys::request_animation_frame_loop(iterate, ctx_ptr) };
}
