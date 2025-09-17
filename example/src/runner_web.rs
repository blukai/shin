use std::cell::RefCell;
use std::rc::Rc;

use anyhow::{Context as _, anyhow};
use raw_window_handle as rwh;
use window::{Event, Window, WindowAttrs};

use crate::{Context, Handler};

fn panic_hook(info: &std::panic::PanicHookInfo) {
    js::throw_str(&info.to_string());
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        let msg = format!(
            "{level:<5} {file}:{line} > {text}",
            level = record.level(),
            file = record.file().unwrap_or_else(|| record.target()),
            line = record
                .line()
                .map_or_else(|| "??".to_string(), |line| line.to_string()),
            text = record.args(),
        );
        js::GLOBAL
            .get("console")
            .get("log")
            .call(&[js::Value::from_str(msg.as_str())])
            .expect("could not log");
    }

    fn flush(&self) {}
}

impl Logger {
    fn init() {
        log::set_logger(&Logger).expect("could not set logger");
        log::set_max_level(log::LevelFilter::Trace);
    }
}

struct GraphicsContext {
    gl_api: gl::Api,
}

impl GraphicsContext {
    fn new(window_handle: rwh::WindowHandle) -> anyhow::Result<Self> {
        let web_window_handle = match window_handle.as_raw() {
            rwh::RawWindowHandle::Web(web) => web,
            _ => return Err(anyhow!(format!("unsupported window: {window_handle:?}"))),
        };
        let selector = format!("canvas[data-raw-handle=\"{}\"]", web_window_handle.id);
        let gl_api = gl::Api::from_selector(selector.as_str()).context("could not load gl api")?;
        Ok(Self { gl_api })
    }
}

struct WebContext<H: Handler + 'static> {
    window: Box<dyn Window>,
    graphics_context: GraphicsContext,
    app_handler: H,
    events: Vec<Event>,
}

impl<H: Handler + 'static> WebContext<H> {
    fn new(window_attrs: WindowAttrs) -> anyhow::Result<Self> {
        let mut window = window::create_window(window_attrs).context("could not create window")?;

        let window_handle = window
            .window_handle()
            .context("window handle is unavailable")?;
        let mut graphics_context =
            GraphicsContext::new(window_handle).context("could not create graphics context")?;

        let app_handler = H::create(Context {
            window: window.as_mut(),
            gl_api: &mut graphics_context.gl_api,
        });

        Ok(Self {
            window,
            graphics_context,
            app_handler,
            events: Vec::new(),
        })
    }

    fn iterate(&mut self) -> anyhow::Result<()> {
        self.window.pump_events()?;

        while let Some(event) = self.window.pop_event() {
            self.events.push(event);
        }

        // TODO: would this drainage cause any problems?
        // probably not? the fact that this is a drain means it can be iterated just once, but you
        // shouldn't need to iterate more then once.
        let events = self.events.drain(..);

        self.app_handler.iterate(
            Context {
                window: self.window.as_mut(),
                gl_api: &mut self.graphics_context.gl_api,
            },
            events,
        );

        Ok(())
    }
}

fn request_animation_frame_loop<H: Handler + 'static>(
    ctx: Rc<RefCell<WebContext<H>>>,
) -> anyhow::Result<()> {
    let cb = Rc::<js::Closure<dyn FnMut()>>::new_uninit();
    let request_animation_frame = js::GLOBAL.get("requestAnimationFrame");
    let closure = js::Closure::new({
        let cb = unsafe { Rc::clone(&cb).assume_init() };
        let ctx = Rc::clone(&ctx);
        let request_animation_frame = request_animation_frame.clone();
        move || {
            let mut ctx = ctx.borrow_mut();
            if let Err(err) = ctx.iterate() {
                log::error!("could not iterate: {}", err);
                return;
            }
            if let Err(err) = request_animation_frame.call(&[js::Value::from_closure(&cb)]) {
                log::error!("could not request animation frame: {}", err);
            }
        }
    });
    let cb = unsafe {
        let ptr = Rc::as_ptr(&cb) as *mut js::Closure<dyn FnMut()>;
        ptr.write(closure);
        cb.assume_init()
    };
    if let Err(err) = request_animation_frame.call(&[js::Value::from_closure(cb.as_ref())]) {
        log::error!("could not request animation frame: {}", err);
    }
    Ok(())
}

pub fn run<H: Handler + 'static>(window_attrs: WindowAttrs) {
    std::panic::set_hook(Box::new(panic_hook));
    Logger::init();

    let ctx = Rc::new(RefCell::new(
        WebContext::<H>::new(window_attrs).expect("could not create app context"),
    ));

    request_animation_frame_loop(ctx).expect("could not start animation loop");
}
