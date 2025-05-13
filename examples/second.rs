use std::cell::RefCell;
use std::ffi::c_void;
use std::panic;

use anyhow::Context as _;
use window::{Window, WindowAttrs};

mod web {
    use std::ffi::{c_char, c_void, CString};
    use std::panic;

    unsafe extern "C" {
        pub fn panic(msg: *const c_char);
        pub fn console_log(msg: *const c_char);
        pub fn request_animation_frame_loop(
            f: unsafe extern "C" fn(*mut c_void) -> bool,
            ctx: *mut c_void,
        );
    }

    pub fn panic_hook(info: &panic::PanicHookInfo) {
        let msg = CString::new(info.to_string()).expect("invalid panic info");
        unsafe { panic(msg.as_ptr()) };
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
            unsafe { console_log(msg.as_ptr()) };
        }

        fn flush(&self) {}
    }

    impl ConsoleLogger {
        pub fn init() {
            log::set_logger(&ConsoleLogger).expect("could not set logger");
            log::set_max_level(log::LevelFilter::Info);
        }
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
    panic::set_hook(Box::new(web::panic_hook));
    web::ConsoleLogger::init();

    let mut ctx = Box::new(Context::new().expect("could not create context"));

    unsafe extern "C" fn iterate(ctx: *mut c_void) -> bool {
        let ctx = unsafe { &mut *(ctx as *mut Context) };
        ctx.iterate().expect("iteration failure");
        return true;
    }
    let ctx_ptr = ctx.as_mut() as *mut Context as *mut c_void;
    unsafe { web::request_animation_frame_loop(iterate, ctx_ptr) };
}
