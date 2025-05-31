use std::collections::VecDeque;

use anyhow::{Context, anyhow};
use raw_window_handle as rwh;
use winit::platform::pump_events::EventLoopExtPumpEvents;

use crate::{
    DEFAULT_LOGICAL_SIZE, Event, PointerButton, PointerButtons, PointerEvent, PointerEventKind,
    Window, WindowAttrs, WindowEvent,
};

#[inline]
fn map_pointer_button(button: winit::event::MouseButton) -> Option<PointerButton> {
    use winit::event::MouseButton;
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Tertiary),
        _ => None,
    }
}

struct App {
    window_attrs: WindowAttrs,

    window: Option<winit::window::Window>,
    window_create_error: Option<winit::error::OsError>,

    pointer_position: (f32, f32),
    pointer_buttons: PointerButtons,

    events: VecDeque<Event>,
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

        let window_attrs = winit::window::WindowAttributes::default()
            .with_inner_size(winit::dpi::LogicalSize::new(
                logical_size.0 as f64,
                logical_size.1 as f64,
            ))
            .with_resizable(self.window_attrs.resizable);
        match event_loop.create_window(window_attrs) {
            Ok(window) => self.window = Some(window),
            Err(err) => self.window_create_error = Some(err),
        }

        self.events
            .push_back(Event::Window(WindowEvent::Configure { logical_size }));

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

        use winit::event::WindowEvent::*;
        let maybe_event = match window_event {
            Resized(physical_size) => Some(Event::Window(WindowEvent::Resize {
                physical_size: (physical_size.width, physical_size.height),
            })),
            CursorMoved { position, .. } => {
                let prev_pos = self.pointer_position;
                let next_pos = (position.x as f32, position.y as f32);
                let delta = (next_pos.0 - prev_pos.0, next_pos.1 - prev_pos.1);

                self.pointer_position = next_pos;
                Some(Event::Pointer(PointerEvent {
                    kind: PointerEventKind::Motion { delta },
                    position: next_pos,
                    buttons: self.pointer_buttons,
                }))
            }
            MouseInput { state, button, .. } => {
                if let Some(button) = map_pointer_button(button) {
                    let pressed = state.is_pressed();

                    self.pointer_buttons.set(button, pressed);
                    Some(Event::Pointer(PointerEvent {
                        kind: if pressed {
                            PointerEventKind::Press { button }
                        } else {
                            PointerEventKind::Release { button }
                        },
                        position: self.pointer_position,
                        buttons: self.pointer_buttons,
                    }))
                } else {
                    None
                }
            }

            CloseRequested => Some(Event::Window(WindowEvent::CloseRequested)),
            other => {
                log::debug!("unused window event: {other:?}");
                None
            }
        };
        if let Some(event) = maybe_event {
            self.events.push_back(event);
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

                pointer_position: (0.0, 0.0),
                pointer_buttons: PointerButtons::default(),

                events: VecDeque::new(),
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

    fn pop_event(&mut self) -> Option<Event> {
        self.app.events.pop_back()
    }
}
