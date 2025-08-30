use std::ffi::c_void;

use raw_window_handle as rwh;
use window::{Event, Window, WindowAttrs, WindowEvent};

use crate::{AppContext, AppHandler};

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        println!(
            "{level:<5} {file}:{line} > {text}",
            level = record.level(),
            file = record.file().unwrap_or_else(|| record.target()),
            line = record
                .line()
                .map_or_else(|| "??".to_string(), |line| line.to_string()),
            text = record.args(),
        );
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
    egl_context: gl::context_egl::Context,
    egl_surface: gl::context_egl::Surface,
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
        display_handle: rwh::DisplayHandle,
        window_handle: rwh::WindowHandle,
        width: u32,
        height: u32,
    ) -> anyhow::Result<&mut InitializedGraphicsContext> {
        assert!(matches!(self, Self::Uninit));

        let egl_context = gl::context_egl::Context::new(
            display_handle,
            gl::context_egl::Config {
                min_swap_interval: Some(0),
                ..gl::context_egl::Config::default()
            },
        )?;
        let egl_surface =
            gl::context_egl::Surface::new(&egl_context, window_handle, width, height)?;

        // TODO: shouldn't need surface here.
        egl_context.make_current(egl_surface.as_ptr())?;

        // TODO: figure out an okay way to include vsync toggle.
        // context.set_swap_interval(&egl, 0)?;

        let gl_api = unsafe {
            gl::api::Api::load_with(|procname| {
                egl_context.get_proc_address(procname) as *mut c_void
            })
        };

        *self = Self::Initialized(InitializedGraphicsContext {
            egl_context,
            egl_surface,
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
    events: Vec<Event>,
    app_handler: Option<A>,
    close_requested: bool,
}

impl<A: AppHandler> Context<A> {
    fn new(window_attrs: WindowAttrs) -> anyhow::Result<Self> {
        let window = window::create_window(window_attrs)?;
        let graphics_context = GraphicsContext::new_uninit();
        Ok(Self {
            window,
            graphics_context,
            events: Vec::new(),
            app_handler: None,
            close_requested: false,
        })
    }

    fn iterate(&mut self) -> anyhow::Result<()> {
        self.window.pump_events()?;

        while let Some(event) = self.window.pop_event() {
            match event {
                Event::Window(WindowEvent::Configure { logical_size }) => {
                    match self.graphics_context {
                        GraphicsContext::Uninit => {
                            let igc = self.graphics_context.init(
                                self.window.display_handle()?,
                                self.window.window_handle()?,
                                logical_size.0,
                                logical_size.1,
                            )?;

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
                Event::Window(WindowEvent::Resized { physical_size }) => {
                    if let GraphicsContext::Initialized(ref mut igc) = self.graphics_context {
                        igc.egl_surface.resize(physical_size.0, physical_size.1)?;
                    }
                }
                Event::Window(WindowEvent::CloseRequested) => {
                    self.close_requested = true;
                }
                _ => {}
            }
            self.events.push(event);
        }

        // TODO: would this drainage cause any problems?
        // probably not? the fact that this is a drain means it can be iterated just once, but you
        // shouldn't need to iterate more then once.
        let events = self.events.drain(..);

        // TODO: maybe don't do this?
        let (
            Some(app_handler),
            GraphicsContext::Initialized(InitializedGraphicsContext {
                egl_context,
                egl_surface,
                gl_api,
            }),
        ) = (self.app_handler.as_mut(), &mut self.graphics_context)
        else {
            return Ok(());
        };

        egl_context.make_current(egl_surface.as_ptr())?;

        app_handler.iterate(
            AppContext {
                window: self.window.as_mut(),
                gl_api,
            },
            events,
        );

        egl_context.swap_buffers(egl_surface.as_ptr())?;

        Ok(())
    }
}

pub fn run<A: AppHandler>(window_attrs: WindowAttrs) {
    Logger::init();

    let mut ctx = Context::<A>::new(window_attrs).expect("could not create app context");
    while !ctx.close_requested {
        ctx.iterate().expect("iteration failure");
    }
}
