use std::ffi::c_void;
use std::mem;
use std::ptr::null_mut;

use anyhow::{Context as _, anyhow};
use raw_window_handle as rwh;
use window::{Event, Window, WindowAttrs, WindowEvent};

use crate::{Context, Handler};

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

struct GraphicsContext {
    egl_connection: egl::wrap::Connection,
    egl_context: egl::wrap::Context,
    gl_api: gl::Api,
}

impl GraphicsContext {
    fn new(display_handle: rwh::DisplayHandle) -> anyhow::Result<Self> {
        let mut egl_connection = match display_handle.as_raw() {
            rwh::RawDisplayHandle::Wayland(rdh) => {
                egl::wrap::Connection::from_wayland_display(rdh.display.as_ptr().cast(), None)
                    .context("could not create egl connection")?
            }
            _ => return Err(anyhow!(format!("unsupported display: {display_handle:?}"))),
        };

        let egl_config = {
            use egl::*;

            // 64 seems enough?
            let mut config_attrs = [NONE as EGLint; 64];
            let mut num_config_attrs = 0;
            let mut push_config_attr = |attr: EGLenum, value: EGLint| {
                config_attrs[num_config_attrs] = attr as EGLint;
                num_config_attrs += 1;
                config_attrs[num_config_attrs] = value;
                num_config_attrs += 1;
            };
            push_config_attr(RED_SIZE, 8);
            push_config_attr(GREEN_SIZE, 8);
            push_config_attr(BLUE_SIZE, 8);
            // NOTE: it is important to set EGL_ALPHA_SIZE, it enables transparency
            push_config_attr(ALPHA_SIZE, 8);
            push_config_attr(CONFORMANT, OPENGL_BIT);
            push_config_attr(RENDERABLE_TYPE, OPENGL_BIT);
            // NOTE: EGL_SAMPLE_BUFFERS + EGL_SAMPLES enable some kind of don't care anti aliasing
            push_config_attr(SAMPLE_BUFFERS, 1);
            push_config_attr(SAMPLES, 4);
            // TODO: might need/want MIN_SWAP_INTERVAL and MAX_SWAP_INTERVAL these to disable vsync?
            let mut num_configs = 0;
            if unsafe {
                egl_connection.api.GetConfigs(
                    *egl_connection.display,
                    null_mut(),
                    0,
                    &mut num_configs,
                )
            } == FALSE
            {
                return Err(egl_connection.unwrap_err()).context("could not get num configs");
            }

            let mut configs = vec![unsafe { mem::zeroed() }; num_configs as usize];
            if unsafe {
                egl_connection.api.ChooseConfig(
                    *egl_connection.display,
                    config_attrs.as_ptr() as _,
                    configs.as_mut_ptr(),
                    num_configs,
                    &mut num_configs,
                )
            } == FALSE
            {
                return Err(egl_connection.unwrap_err()).context("could not choose config");
            }
            unsafe { configs.set_len(num_configs as usize) };
            configs
                .first()
                .copied()
                .context("could not choose config (no compatible ones probably)")?
        };

        let egl_context = egl_connection.create_context(
            egl::OPENGL_API,
            egl_config,
            None,
            Some(&[
                egl::CONTEXT_MAJOR_VERSION as egl::EGLint,
                3,
                egl::NONE as egl::EGLint,
            ]),
        )?;

        if unsafe {
            egl_connection.api.MakeCurrent(
                *egl_connection.display,
                egl::NO_SURFACE,
                egl::NO_SURFACE,
                egl_context.context,
            )
        } == egl::FALSE
        {
            return Err(egl_connection.unwrap_err()).context("could not make current");
        }

        // TODO: figure out an okay way to include vsync toggle.
        // context.set_swap_interval(&egl, 0)?;

        let gl_api = unsafe {
            gl::Api::load_with(|procname| {
                egl_connection.api.GetProcAddress(procname) as *mut c_void
            })
        };

        Ok(Self {
            egl_connection,
            egl_context,
            gl_api,
        })
    }
}

struct NativeContext<H: Handler + 'static> {
    window: Box<dyn Window>,
    graphics_context: GraphicsContext,
    // NOTE: surface does not belong to graphics context because a single app (not this one) can
    // have multiple windows and thus multiple surfaces and all surfaces can (and must?) be created
    // by a single context.
    egl_window_surface: egl::wrap::WindowSurface,
    events: Vec<Event>,
    app_handler: H,
    close_requested: bool,
}

impl<H: Handler + 'static> NativeContext<H> {
    fn new(window_attrs: WindowAttrs) -> anyhow::Result<Self> {
        let mut window = window::create_window(window_attrs).context("could not create window")?;

        let display_handle = window
            .display_handle()
            .context("display handle is unavailable")?;
        let mut graphics_context =
            GraphicsContext::new(display_handle).context("could not create graphics context")?;

        let window_handle = window
            .window_handle()
            .context("window handle is unavailable")?;
        let physical_size = window.physical_size();
        let egl_window_surface = match window_handle.as_raw() {
            rwh::RawWindowHandle::Wayland(rwh) => {
                graphics_context.egl_connection.create_wayland_surface(
                    graphics_context.egl_context.config,
                    rwh.surface.as_ptr().cast(),
                    physical_size.0,
                    physical_size.1,
                    None,
                )?
            }
            other => return Err(anyhow!("unsupported window system: {other:?}")),
        };

        let app_handler = H::create(Context {
            window: window.as_mut(),
            gl_api: &mut graphics_context.gl_api,
        });

        Ok(Self {
            window,
            graphics_context,
            egl_window_surface,
            events: Vec::new(),
            app_handler,
            close_requested: false,
        })
    }

    fn iterate(&mut self) -> anyhow::Result<()> {
        self.window.pump_events()?;

        while let Some(event) = self.window.pop_event() {
            match event {
                Event::Window(WindowEvent::Resized { physical_size }) => {
                    self.egl_window_surface
                        .resize(physical_size.0, physical_size.1);
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

        let gc = &mut self.graphics_context;

        if unsafe {
            gc.egl_connection.api.MakeCurrent(
                *gc.egl_connection.display,
                self.egl_window_surface.surface,
                self.egl_window_surface.surface,
                gc.egl_context.context,
            )
        } == egl::FALSE
        {
            return Err(gc.egl_connection.unwrap_err()).context("could not make current");
        }

        self.app_handler.iterate(
            Context {
                window: self.window.as_mut(),
                gl_api: &mut gc.gl_api,
            },
            events,
        );

        if unsafe {
            gc.egl_connection
                .api
                .SwapBuffers(*gc.egl_connection.display, self.egl_window_surface.surface)
        } == egl::FALSE
        {
            return Err(gc.egl_connection.unwrap_err()).context("could not swap buffers");
        }

        Ok(())
    }
}

pub fn run<H: Handler + 'static>(window_attrs: WindowAttrs) {
    Logger::init();

    let mut ctx = NativeContext::<H>::new(window_attrs).expect("could not create app context");
    while !ctx.close_requested {
        ctx.iterate().expect("iteration failure");
    }
}
