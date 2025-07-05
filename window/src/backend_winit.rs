use std::collections::VecDeque;

use anyhow::{Context, anyhow};
use input::{CursorShape, KeyboardEvent, Keycode, PointerButton, PointerEvent, Scancode};
use raw_window_handle as rwh;
use winit::platform::pump_events::EventLoopExtPumpEvents;

use crate::{DEFAULT_LOGICAL_SIZE, Event, Window, WindowAttrs, WindowEvent};

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

#[inline]
fn map_cursor_shape(cursor_shape: CursorShape) -> winit::window::Cursor {
    use winit::window::{Cursor, CursorIcon};
    Cursor::Icon(match cursor_shape {
        CursorShape::Default => CursorIcon::Default,
        CursorShape::Pointer => CursorIcon::Pointer,
    })
}

#[inline]
fn map_keyboard_physical_key(physical_key: winit::keyboard::PhysicalKey) -> Option<Scancode> {
    use winit::keyboard::{KeyCode, PhysicalKey};
    match physical_key {
        PhysicalKey::Code(KeyCode::Escape) => Some(Scancode::Esc),
        PhysicalKey::Code(KeyCode::KeyW) => Some(Scancode::W),
        PhysicalKey::Code(KeyCode::KeyA) => Some(Scancode::A),
        PhysicalKey::Code(KeyCode::KeyS) => Some(Scancode::S),
        PhysicalKey::Code(KeyCode::KeyD) => Some(Scancode::D),
        PhysicalKey::Code(KeyCode::ShiftLeft) => Some(Scancode::ShiftLeft),
        PhysicalKey::Code(KeyCode::ShiftRight) => Some(Scancode::ShiftRight),
        PhysicalKey::Code(KeyCode::ArrowUp) => Some(Scancode::ArrowUp),
        PhysicalKey::Code(KeyCode::ArrowLeft) => Some(Scancode::ArrowLeft),
        PhysicalKey::Code(KeyCode::ArrowRight) => Some(Scancode::ArrowRight),
        PhysicalKey::Code(KeyCode::ArrowDown) => Some(Scancode::ArrowDown),
        _ => None,
    }
}

struct App {
    window_attrs: WindowAttrs,

    window: Option<winit::window::Window>,
    window_create_error: Option<winit::error::OsError>,

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
            Resized(physical_size) => Some(Event::Window(WindowEvent::Resized {
                physical_size: (physical_size.width, physical_size.height),
            })),
            ScaleFactorChanged { scale_factor, .. } => {
                Some(Event::Window(WindowEvent::ScaleFactorChanged {
                    scale_factor,
                }))
            }
            CursorMoved { position, .. } => {
                let position = (position.x, position.y);
                Some(Event::Pointer(PointerEvent::Motion { position }))
            }
            MouseInput { button, state, .. } => {
                if let Some(button) = map_pointer_button(button) {
                    let pressed = state.is_pressed();
                    Some(Event::Pointer(if pressed {
                        PointerEvent::Press { button }
                    } else {
                        PointerEvent::Release { button }
                    }))
                } else {
                    None
                }
            }
            KeyboardInput { event, .. } => {
                if let Some(scancode) = map_keyboard_physical_key(event.physical_key) {
                    let keycode = match event.logical_key {
                        winit::keyboard::Key::Character(str) if str.chars().count() == 1 => {
                            Keycode::Char(str.chars().next().unwrap())
                        }
                        _ => Keycode::Unhandled,
                    };
                    let pressed = event.state.is_pressed();
                    Some(Event::Keyboard(if pressed {
                        KeyboardEvent::Press { scancode, keycode }
                    } else {
                        KeyboardEvent::Release { scancode, keycode }
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
        self.app.events.pop_front()
    }

    fn set_cursor_shape(&mut self, cursor_shape: CursorShape) -> anyhow::Result<()> {
        if let Some(ref mut window) = self.app.window {
            window.set_cursor(map_cursor_shape(cursor_shape));
        }
        Ok(())
    }

    fn scale_factor(&self) -> f64 {
        let window = self.app.window.as_ref().expect("initialized window");
        window.scale_factor()
    }

    fn size(&self) -> (u32, u32) {
        let window = self.app.window.as_ref().expect("initialized window");
        let inner_size = window.inner_size();
        (inner_size.width, inner_size.height)
    }
}
