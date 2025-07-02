use std::{ffi::CString, panic};

use raw_window_handle as rwh;
use window::{Event, Window, WindowAttrs, WindowEvent};

use crate::{AppContext, AppHandler};

fn panic_hook(info: &panic::PanicHookInfo) {
    let msg = CString::new(info.to_string()).expect("invalid panic info");
    unsafe { window::js_sys::panic(msg.as_ptr()) };
}

struct Logger;

impl log::Log for Logger {
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

impl Logger {
    fn init() {
        log::set_logger(&Logger).expect("could not set logger");
        log::set_max_level(log::LevelFilter::Trace);
    }
}

struct InitializedGraphicsContext {
    web_surface: gl::context_web::Surface,
    gl_api: gl::api::Api,
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

        let web_surface = gl::context_web::Surface::new(window_handle);

        let gl_api = gl::api::Api::from_extern_ref(web_surface.as_extern_ref());

        *self = Self::Initialized(InitializedGraphicsContext {
            web_surface,
            gl_api,
        });
        let Self::Initialized(init) = self else {
            unreachable!();
        };
        Ok(init)
    }
}

struct Context<A: AppHandler> {
    window: Box<dyn Window>,
    graphics_context: GraphicsContext,
    app_handler: Option<A>,
}

impl<A: AppHandler> Context<A> {
    fn new(window_attrs: WindowAttrs) -> anyhow::Result<Self> {
        let window = window::create_window(window_attrs)?;
        let graphics_context = GraphicsContext::new_uninit();

        Ok(Self {
            window,
            graphics_context,
            app_handler: None,
        })
    }

    fn iterate(&mut self) -> anyhow::Result<()> {
        self.window.pump_events()?;

        while let Some(event) = self.window.pop_event() {
            if !matches!(event, Event::Pointer(input::PointerEvent::Motion { .. })) {
                log::debug!("event: {event:?}");
            }

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

                    continue;
                }

                _ => {}
            }

            let (
                Some(app_handler),
                GraphicsContext::Initialized(InitializedGraphicsContext { gl_api, .. }),
            ) = (self.app_handler.as_mut(), &mut self.graphics_context)
            else {
                continue;
            };
            app_handler.handle_event(
                AppContext {
                    window: self.window.as_mut(),
                    // TODO: should gl be included into event context? prob not.
                    gl_api,
                },
                event,
            );
        }

        let (
            Some(app_handler),
            GraphicsContext::Initialized(InitializedGraphicsContext { gl_api, .. }),
        ) = (self.app_handler.as_mut(), &mut self.graphics_context)
        else {
            return Ok(());
        };

        app_handler.update(AppContext {
            window: self.window.as_mut(),
            gl_api,
        });

        Ok(())
    }
}

pub fn run<A: AppHandler>(window_attrs: WindowAttrs) {
    std::panic::set_hook(Box::new(panic_hook));
    Logger::init();

    // TODO: figure out wasm-side lifetime of the entire thing
    let ctx = Box::new(Context::<A>::new(window_attrs).expect("could not create app context"));
    let mut ctx = std::mem::ManuallyDrop::new(ctx);

    unsafe extern "C" fn iterate<A: AppHandler>(ctx: *mut std::ffi::c_void) -> bool {
        let ctx = unsafe { &mut *(ctx as *mut Context<A>) };
        ctx.iterate().expect("iteration failure");
        return true;
    }
    let ctx_ptr = ctx.as_mut() as *mut Context<A> as *mut std::ffi::c_void;
    unsafe { window::js_sys::request_animation_frame_loop(iterate::<A>, ctx_ptr) };
}
