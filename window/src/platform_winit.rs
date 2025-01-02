use std::collections::VecDeque;

use anyhow::{anyhow, Context};
use raw_window_handle as rwh;
use winit::platform::pump_events::EventLoopExtPumpEvents;

use crate::{Event, Size, Window, WindowConfig, DEFAULT_LOGICAL_SIZE};

struct WinitApp {
    window_config: WindowConfig,
    window: Option<winit::window::Window>,
    window_create_error: Option<winit::error::OsError>,

    events: VecDeque<Event>,
}

pub struct WinitWindow {
    event_loop: winit::event_loop::EventLoop<()>,
    app: WinitApp,
}

impl winit::application::ApplicationHandler for WinitApp {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let logical_size = self
            .window_config
            .logical_size
            .unwrap_or(DEFAULT_LOGICAL_SIZE);

        let attrs = winit::window::WindowAttributes::default().with_inner_size(
            winit::dpi::LogicalSize::new(logical_size.width as f64, logical_size.height as f64),
        );
        match event_loop.create_window(attrs) {
            Ok(window) => self.window = Some(window),
            Err(err) => self.window_create_error = Some(err),
        }

        let event = Event::Configure { logical_size };
        self.events.push_back(event);

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

        use winit::event::WindowEvent;
        let maybe_event = match window_event {
            WindowEvent::Resized(physical_size) => Some(Event::Configure {
                logical_size: {
                    // TODO: i probably should switch to physical size everywhere
                    let logical_size = physical_size.to_logical(1.0);
                    Size::new(logical_size.width, logical_size.height)
                },
            }),
            WindowEvent::CloseRequested => Some(Event::CloseRequested),
            window_event => {
                log::debug!("unused window event: {window_event:?}");
                None
            }
        };
        if let Some(event) = maybe_event {
            self.events.push_back(event);
        }
    }
}

impl WinitWindow {
    pub fn new(window_config: WindowConfig) -> anyhow::Result<Self> {
        let this = Self {
            event_loop: winit::event_loop::EventLoop::new()?,
            app: WinitApp {
                window_config,
                window: None,
                window_create_error: None,

                events: VecDeque::new(),
            },
        };
        Ok(this)
    }
}

impl rwh::HasDisplayHandle for WinitWindow {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        self.event_loop.display_handle()
    }
}

impl rwh::HasWindowHandle for WinitWindow {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        if let Some(ref window) = self.app.window {
            window.window_handle()
        } else {
            Err(rwh::HandleError::Unavailable)
        }
    }
}

impl Window for WinitWindow {
    fn update(&mut self) -> anyhow::Result<()> {
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

    fn pop_event(&mut self) -> Option<Event> {
        self.app.events.pop_back()
    }
}
