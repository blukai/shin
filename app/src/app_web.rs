use std::{cell::RefCell, rc::Rc};

use anyhow::{Context as _, anyhow};
use raw_window_handle as rwh;
use window::{Event, Window, WindowAttrs, WindowEvent};

use crate::{AppContext, AppHandler};

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

struct InitializedGraphicsContext {
    gl_api: gl::Api,
}

enum GraphicsContext {
    Initialized(InitializedGraphicsContext),
    Uninit,
}

impl GraphicsContext {
    fn new_uninit() -> Self {
        Self::Uninit
    }

    fn init(
        &mut self,
        window_handle: rwh::WindowHandle,
    ) -> anyhow::Result<&mut InitializedGraphicsContext> {
        assert!(matches!(self, Self::Uninit));

        let web_window_handle = match window_handle.as_raw() {
            rwh::RawWindowHandle::Web(web) => web,
            _ => {
                return Err(anyhow!(format!(
                    "unsupported window system (window handle: {window_handle:?})"
                )));
            }
        };
        let gl_api =
            gl::Api::from_web_window_handle(web_window_handle).context("could not load gl api")?;

        *self = Self::Initialized(InitializedGraphicsContext { gl_api });
        let Self::Initialized(init) = self else {
            unreachable!();
        };
        Ok(init)
    }
}

struct Context<A: AppHandler + 'static> {
    window: Box<dyn Window>,
    graphics_context: GraphicsContext,
    events: Vec<Event>,
    app_handler: Option<A>,
}

impl<A: AppHandler + 'static> Context<A> {
    fn new(window_attrs: WindowAttrs) -> anyhow::Result<Self> {
        let window = window::create_window(window_attrs)?;
        let graphics_context = GraphicsContext::new_uninit();

        Ok(Self {
            window,
            graphics_context,
            events: Vec::new(),
            app_handler: None,
        })
    }

    fn iterate(&mut self) -> anyhow::Result<()> {
        self.window.pump_events()?;

        while let Some(event) = self.window.pop_event() {
            match event {
                Event::Window(WindowEvent::Configure { logical_size: _ }) => {
                    match self.graphics_context {
                        GraphicsContext::Uninit => {
                            let igc = self.graphics_context.init(self.window.window_handle()?)?;
                            self.app_handler = Some(A::create(AppContext {
                                window: self.window.as_mut(),
                                gl_api: &mut igc.gl_api,
                            }));
                        }
                        GraphicsContext::Initialized(_) => {
                            unreachable!();
                        }
                    }
                }
                _ => {}
            }
            self.events.push(event);
        }

        // TODO: would this drainage cause any problems?
        // probably not? the fact that this is a drain means it can be iterated just once, but you
        // shouldn't need to iterate more then once.
        let events = self.events.drain(..);

        let (
            Some(app_handler),
            GraphicsContext::Initialized(InitializedGraphicsContext { gl_api, .. }),
        ) = (self.app_handler.as_mut(), &mut self.graphics_context)
        else {
            return Ok(());
        };

        app_handler.iterate(
            AppContext {
                window: self.window.as_mut(),
                gl_api,
            },
            events,
        );

        Ok(())
    }
}

fn request_animation_frame_loop<A: AppHandler + 'static>(
    ctx: Rc<RefCell<Context<A>>>,
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

pub fn run<A: AppHandler + 'static>(window_attrs: WindowAttrs) {
    std::panic::set_hook(Box::new(panic_hook));
    Logger::init();

    let ctx = Rc::new(RefCell::new(
        Context::<A>::new(window_attrs).expect("could not create app context"),
    ));

    request_animation_frame_loop(ctx).expect("could not start animation loop");
}
