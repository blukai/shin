use gpu::gl::{self, GlContexter};
use platform::InitializedGraphicsContext;
use raw_window_handle::{HasDisplayHandle as _, HasWindowHandle as _};
use window::{Event, Window, WindowAttrs, WindowEvent};

#[cfg(unix)]
mod platform {
    use std::ffi::c_void;

    use gpu::{egl, gl};
    use raw_window_handle as rwh;

    pub struct Logger;

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
        pub fn init() {
            log::set_logger(&Logger).expect("could not set logger");
            log::set_max_level(log::LevelFilter::Trace);
        }
    }

    pub struct InitializedGraphicsContext {
        pub context: egl::Context,
        pub surface: egl::Surface,
        pub gl: gl::Context,
    }

    pub enum GraphicsContext {
        Initialized(InitializedGraphicsContext),
        Uninit,
    }

    impl GraphicsContext {
        pub fn new_uninit() -> Self {
            Self::Uninit
        }

        pub fn init(
            &mut self,
            display_handle: rwh::DisplayHandle,
            window_handle: rwh::WindowHandle,
            width: u32,
            height: u32,
        ) -> anyhow::Result<&mut InitializedGraphicsContext> {
            assert!(matches!(self, Self::Uninit));

            let context = egl::Context::new(
                display_handle,
                egl::Config {
                    min_swap_interval: Some(0),
                    ..egl::Config::default()
                },
            )?;
            let surface = egl::Surface::new(&context, window_handle, width, height)?;

            // TODO: figure out an okay way to include vsync toggle.
            // context.make_current(&egl, egl_surface.as_ptr())?;
            // context.set_swap_interval(&egl, 0)?;

            let gl = unsafe {
                gl::Context::load_with(|procname| context.get_proc_address(procname) as *mut c_void)
            };

            *self = Self::Initialized(InitializedGraphicsContext {
                context,
                surface,
                gl,
            });
            let Self::Initialized(init) = self else {
                unreachable!();
            };
            Ok(init)
        }
    }
}

#[cfg(target_family = "wasm")]
mod platform {
    use std::{ffi::CString, panic};

    use gpu::{gl, web};
    use raw_window_handle as rwh;

    pub fn panic_hook(info: &panic::PanicHookInfo) {
        let msg = CString::new(info.to_string()).expect("invalid panic info");
        unsafe { window::js_sys::panic(msg.as_ptr()) };
    }

    pub struct Logger;

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
        pub fn init() {
            log::set_logger(&Logger).expect("could not set logger");
            log::set_max_level(log::LevelFilter::Trace);
        }
    }

    pub struct InitializedGraphicsContext {
        pub surface: web::Surface,
        pub gl: gl::Context,
    }

    pub enum GraphicsContext {
        Initialized(InitializedGraphicsContext),
        Uninit,
    }

    impl GraphicsContext {
        pub fn new_uninit() -> Self {
            Self::Uninit
        }

        pub fn init(
            &mut self,
            display_handle: rwh::DisplayHandle,
            window_handle: rwh::WindowHandle,
        ) -> anyhow::Result<&mut InitializedGraphicsContext> {
            assert!(matches!(self, Self::Uninit));

            let surface = web::Surface::new(window_handle);

            let gl = unsafe { gl::Context::from_extern_ref(surface.as_extern_ref()) };

            *self = Self::Initialized(InitializedGraphicsContext { surface, gl });

            let Self::Initialized(init) = self else {
                unreachable!();
            };
            Ok(init)
        }
    }
}

struct Context {
    window: Box<dyn Window>,
    window_size: (u32, u32),
    graphics_context: platform::GraphicsContext,
    close_requested: bool,
}

impl Context {
    fn new() -> anyhow::Result<Self> {
        let window = window::create_window(WindowAttrs::default())?;
        let graphics_context = platform::GraphicsContext::new_uninit();

        Ok(Self {
            window,
            window_size: (0, 0),
            graphics_context,
            close_requested: false,
        })
    }

    fn iterate(&mut self) -> anyhow::Result<()> {
        self.window.pump_events()?;

        while let Some(event) = self.window.pop_event() {
            log::debug!("event: {event:?}");

            match event {
                Event::Window(WindowEvent::Configure { logical_size }) => {
                    self.window_size = logical_size;

                    match self.graphics_context {
                        platform::GraphicsContext::Uninit => {
                            self.graphics_context.init(
                                self.window.display_handle()?,
                                self.window.window_handle()?,
                                #[cfg(unix)]
                                logical_size.0,
                                #[cfg(unix)]
                                logical_size.1,
                            )?;
                        }
                        platform::GraphicsContext::Initialized(ref mut igc) => {
                            #[cfg(unix)]
                            igc.surface.resize(logical_size.0, logical_size.1)?;
                        }
                    }
                }
                Event::Window(WindowEvent::Resize { physical_size }) => {
                    self.window_size = physical_size;

                    #[cfg(target_family = "wasm")]
                    unreachable!();

                    #[cfg(unix)]
                    if let platform::GraphicsContext::Initialized(ref mut igc) =
                        self.graphics_context
                    {
                        igc.surface.resize(physical_size.0, physical_size.1)?;
                    }
                }
                Event::Window(WindowEvent::CloseRequested) => {
                    self.close_requested = true;
                    return Ok(());
                }
                _ => {}
            }
        }

        if let platform::GraphicsContext::Initialized(InitializedGraphicsContext {
            #[cfg(unix)]
            context,
            surface,
            gl,
            ..
        }) = &self.graphics_context
        {
            unsafe {
                #[cfg(unix)]
                context.make_current(surface.as_ptr())?;

                gl.clear_color(1.0, 0.0, 0.0, 1.0);
                gl.clear(gl::COLOR_BUFFER_BIT);

                #[cfg(unix)]
                context.swap_buffers(surface.as_ptr())?;
            }
        }

        Ok(())
    }
}

fn main() {
    #[cfg(unix)]
    {
        platform::Logger::init();
    }

    #[cfg(target_family = "wasm")]
    {
        std::panic::set_hook(Box::new(platform::panic_hook));
        platform::Logger::init();
    }

    // TODO: figure out wasm-side lifetime of the entire thing
    let ctx = Box::new(Context::new().expect("could not create context"));
    let mut ctx = std::mem::ManuallyDrop::new(ctx);

    #[cfg(unix)]
    {
        while !ctx.close_requested {
            ctx.iterate().expect("iteration failure");
        }
    }

    #[cfg(target_family = "wasm")]
    unsafe {
        unsafe extern "C" fn iterate(ctx: *mut std::ffi::c_void) -> bool {
            let ctx = unsafe { &mut *(ctx as *mut Context) };
            ctx.iterate().expect("iteration failure");
            return true;
        }
        let ctx_ptr = ctx.as_mut() as *mut Context as *mut std::ffi::c_void;
        window::js_sys::request_animation_frame_loop(iterate, ctx_ptr)
    };
}
