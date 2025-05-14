use std::ffi::{c_void, CString};
use std::panic;

use anyhow::Context as _;
use window::{Window, WindowAttrs};

pub fn panic_hook(info: &panic::PanicHookInfo) {
    let msg = CString::new(info.to_string()).expect("invalid panic info");
    unsafe { window::js_bindings::panic(msg.as_ptr()) };
}

pub struct ConsoleLogger;

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
        unsafe { window::js_bindings::console_log(msg.as_ptr()) };
    }

    fn flush(&self) {}
}

impl ConsoleLogger {
    pub fn init() {
        log::set_logger(&ConsoleLogger).expect("could not set logger");
        log::set_max_level(log::LevelFilter::Info);
    }
}

pub struct Context {
    window: Box<dyn Window>,
    i: usize,
}

impl Context {
    fn new() -> anyhow::Result<Self> {
        let window =
            window::create_window(WindowAttrs::default()).context("could not create window")?;
        Ok(Context { window, i: 0 })
    }

    fn iterate(&mut self) -> anyhow::Result<()> {
        log::info!("iterate {}", self.i);
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
    unsafe { window::js_bindings::request_animation_frame_loop(iterate, ctx_ptr) };
}
