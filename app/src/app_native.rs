use std::{ffi::c_void, mem, ptr::null_mut};

use anyhow::{Context as _, anyhow};
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
    egl_connection: egl::wrap::Connection,
    egl_context: egl::wrap::Context,
    egl_surface: egl::wrap::Surface,
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
        display_handle: rwh::DisplayHandle,
        window_handle: rwh::WindowHandle,
        width: u32,
        height: u32,
    ) -> anyhow::Result<&mut InitializedGraphicsContext> {
        assert!(matches!(self, Self::Uninit));

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

        let egl_surface = match window_handle.as_raw() {
            rwh::RawWindowHandle::Wayland(rwh) => egl_connection.create_wayland_surface(
                egl_context.config,
                rwh.surface.as_ptr().cast(),
                width,
                height,
                None,
            )?,
            other => return Err(anyhow!("unsupported window system: {other:?}")),
        };

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

        *self = Self::Initialized(InitializedGraphicsContext {
            egl_connection,
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
                        igc.egl_surface.resize(physical_size.0, physical_size.1);
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
                egl_connection,
                egl_context,
                egl_surface,
                gl_api,
            }),
        ) = (self.app_handler.as_mut(), &mut self.graphics_context)
        else {
            return Ok(());
        };

        if unsafe {
            egl_connection.api.MakeCurrent(
                *egl_connection.display,
                egl_surface.surface,
                egl_surface.surface,
                egl_context.context,
            )
        } == egl::FALSE
        {
            return Err(egl_connection.unwrap_err()).context("could not make current");
        }

        app_handler.iterate(
            AppContext {
                window: self.window.as_mut(),
                gl_api,
            },
            events,
        );

        if unsafe {
            egl_connection
                .api
                .SwapBuffers(*egl_connection.display, egl_surface.surface)
        } == egl::FALSE
        {
            return Err(egl_connection.unwrap_err()).context("could not swap buffers");
        }

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
