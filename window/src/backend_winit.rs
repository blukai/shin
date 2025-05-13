use std::collections::VecDeque;

use anyhow::{anyhow, Context};
use raw_window_handle as rwh;
use winit::platform::pump_events::EventLoopExtPumpEvents;

use crate::{Window, WindowAttrs, WindowEvent, DEFAULT_LOGICAL_SIZE};

struct App {
    window_attrs: WindowAttrs,

    window: Option<winit::window::Window>,
    window_create_error: Option<winit::error::OsError>,

    window_events: VecDeque<WindowEvent>,
}

pub struct WinitBackend {
    event_loop: winit::event_loop::EventLoop<()>,
    app: App,
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let logical_size = self
            .window_attrs
            .logical_size
            .unwrap_or(DEFAULT_LOGICAL_SIZE);

        let window_attrs = winit::window::WindowAttributes::default().with_inner_size(
            winit::dpi::LogicalSize::new(logical_size.0 as f64, logical_size.1 as f64),
        );
        match event_loop.create_window(window_attrs) {
            Ok(window) => self.window = Some(window),
            Err(err) => self.window_create_error = Some(err),
        }

        let window_event = WindowEvent::Configure { logical_size };
        self.window_events.push_back(window_event);

        log::info!("created winit window");
    }

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        window_event: winit::event::WindowEvent,
    ) {
        let window = self.window.as_ref().unwrap();
        assert!(window.id() == window_id);

        let maybe_window_event = match window_event {
            winit::event::WindowEvent::Resized(physical_size) => Some(WindowEvent::Configure {
                logical_size: {
                    // TODO: i probably should switch to physical size everywhere
                    let logical_size = physical_size.to_logical(1.0);
                    (logical_size.width, logical_size.height)
                },
            }),
            winit::event::WindowEvent::CloseRequested => Some(WindowEvent::CloseRequested),
            window_event => {
                log::debug!("unused window event: {window_event:?}");
                None
            }
        };
        if let Some(window_event) = maybe_window_event {
            self.window_events.push_back(window_event);
        }
    }
}

impl WinitBackend {
    pub fn new(attrs: WindowAttrs) -> anyhow::Result<Self> {
        let this = Self {
            event_loop: winit::event_loop::EventLoop::new()?,
            app: App {
                window_attrs: attrs,

                window: None,
                window_create_error: None,

                window_events: VecDeque::new(),
            },
        };
        Ok(this)
    }
}

impl rwh::HasDisplayHandle for WinitBackend {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        self.event_loop.display_handle()
    }
}

impl rwh::HasWindowHandle for WinitBackend {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        if let Some(ref window) = self.app.window {
            window.window_handle()
        } else {
            Err(rwh::HandleError::Unavailable)
        }
    }
}

impl Window for WinitBackend {
    fn pump_events(&mut self) -> anyhow::Result<()> {
        use winit::platform::pump_events::PumpStatus;
        let ret = match self.event_loop.pump_app_events(None, &mut self.app) {
            PumpStatus::Exit(code) => Err(anyhow!(format!("unexpected exit (code {code})"))),
            PumpStatus::Continue => Ok(()),
        };

        if let Some(err) = self.app.window_create_error.take() {
            return Err(err).context("could not create window");
        }
        assert!(self.app.window.is_some());

        ret
    }

    fn pop_event(&mut self) -> Option<WindowEvent> {
        self.app.window_events.pop_back()
    }
}
