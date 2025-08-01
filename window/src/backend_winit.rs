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
        CursorShape::Text => CursorIcon::Text,
    })
}

#[inline]
fn map_keyboard_physical_key(physical_key: winit::keyboard::PhysicalKey) -> Option<Scancode> {
    use winit::keyboard::{KeyCode, PhysicalKey};
    match physical_key {
        PhysicalKey::Code(keycode) => match keycode {
            // KeyCode::Reserved => Some(Scancode::Reserved),
            KeyCode::Escape => Some(Scancode::Esc),
            KeyCode::Digit1 => Some(Scancode::Num1),
            KeyCode::Digit2 => Some(Scancode::Num2),
            KeyCode::Digit3 => Some(Scancode::Num3),
            KeyCode::Digit4 => Some(Scancode::Num4),
            KeyCode::Digit5 => Some(Scancode::Num5),
            KeyCode::Digit6 => Some(Scancode::Num6),
            KeyCode::Digit7 => Some(Scancode::Num7),
            KeyCode::Digit8 => Some(Scancode::Num8),
            KeyCode::Digit9 => Some(Scancode::Num9),
            KeyCode::Digit0 => Some(Scancode::Num0),
            KeyCode::Minus => Some(Scancode::Minus),
            KeyCode::Equal => Some(Scancode::Equal),
            KeyCode::Backspace => Some(Scancode::Backspace),
            KeyCode::Tab => Some(Scancode::Tab),
            KeyCode::KeyQ => Some(Scancode::Q),
            KeyCode::KeyW => Some(Scancode::W),
            KeyCode::KeyE => Some(Scancode::E),
            KeyCode::KeyR => Some(Scancode::R),
            KeyCode::KeyT => Some(Scancode::T),
            KeyCode::KeyY => Some(Scancode::Y),
            KeyCode::KeyU => Some(Scancode::U),
            KeyCode::KeyI => Some(Scancode::I),
            KeyCode::KeyO => Some(Scancode::O),
            KeyCode::KeyP => Some(Scancode::P),
            KeyCode::BracketLeft => Some(Scancode::BraceLeft),
            KeyCode::BracketRight => Some(Scancode::BraceRight),
            KeyCode::Enter => Some(Scancode::Enter),
            KeyCode::ControlLeft => Some(Scancode::CtrlLeft),
            KeyCode::KeyA => Some(Scancode::A),
            KeyCode::KeyS => Some(Scancode::S),
            KeyCode::KeyD => Some(Scancode::D),
            KeyCode::KeyF => Some(Scancode::F),
            KeyCode::KeyG => Some(Scancode::G),
            KeyCode::KeyH => Some(Scancode::H),
            KeyCode::KeyJ => Some(Scancode::J),
            KeyCode::KeyK => Some(Scancode::K),
            KeyCode::KeyL => Some(Scancode::L),
            KeyCode::Semicolon => Some(Scancode::Semicolon),
            KeyCode::Quote => Some(Scancode::Apostrophe),
            KeyCode::Backquote => Some(Scancode::Grave),
            KeyCode::ShiftLeft => Some(Scancode::ShiftLeft),
            KeyCode::Backslash => Some(Scancode::Backslash),
            KeyCode::KeyZ => Some(Scancode::Z),
            KeyCode::KeyX => Some(Scancode::X),
            KeyCode::KeyC => Some(Scancode::C),
            KeyCode::KeyV => Some(Scancode::V),
            KeyCode::KeyB => Some(Scancode::B),
            KeyCode::KeyN => Some(Scancode::N),
            KeyCode::KeyM => Some(Scancode::M),
            KeyCode::Comma => Some(Scancode::Comma),
            KeyCode::Period => Some(Scancode::Dot),
            KeyCode::Slash => Some(Scancode::Slash),
            KeyCode::ShiftRight => Some(Scancode::ShiftRight),
            // KeyCode::KPAsterisk => Some(Scancode::KPAsterisk),
            KeyCode::AltLeft => Some(Scancode::AltLeft),
            KeyCode::Space => Some(Scancode::Space),
            KeyCode::CapsLock => Some(Scancode::CapsLock),
            // KeyCode::F1 => Some(Scancode::F1),
            // KeyCode::F2 => Some(Scancode::F2),
            // KeyCode::F3 => Some(Scancode::F3),
            // KeyCode::F4 => Some(Scancode::F4),
            // KeyCode::F5 => Some(Scancode::F5),
            // KeyCode::F6 => Some(Scancode::F6),
            // KeyCode::F7 => Some(Scancode::F7),
            // KeyCode::F8 => Some(Scancode::F8),
            // KeyCode::F9 => Some(Scancode::F9),
            // KeyCode::F10 => Some(Scancode::F10),
            KeyCode::NumLock => Some(Scancode::NumLock),
            KeyCode::ScrollLock => Some(Scancode::ScrollLock),
            // KeyCode::KP7 => Some(Scancode::KP7),
            // KeyCode::KP8 => Some(Scancode::KP8),
            // KeyCode::KP9 => Some(Scancode::KP9),
            // KeyCode::KPMinus => Some(Scancode::KPMinus),
            // KeyCode::KP4 => Some(Scancode::KP4),
            // KeyCode::KP5 => Some(Scancode::KP5),
            // KeyCode::KP6 => Some(Scancode::KP6),
            // KeyCode::KPPlus => Some(Scancode::KPPlus),
            // KeyCode::KP1 => Some(Scancode::KP1),
            // KeyCode::KP2 => Some(Scancode::KP2),
            // KeyCode::KP3 => Some(Scancode::KP3),
            // KeyCode::KP0 => Some(Scancode::KP0),
            // KeyCode::KPDOT => Some(Scancode::KPDOT),
            // KeyCode::_ => Some(Scancode::_),
            // KeyCode::ZENKAKUHANKAKU => Some(Scancode::ZENKAKUHANKAKU),
            // KeyCode::102ND => Some(Scancode::102ND),
            // KeyCode::F11 => Some(Scancode::F11),
            // KeyCode::F12 => Some(Scancode::F12),
            // KeyCode::RO => Some(Scancode::RO),
            // KeyCode::KATAKANA => Some(Scancode::KATAKANA),
            // KeyCode::HIRAGANA => Some(Scancode::HIRAGANA),
            // KeyCode::HENKAN => Some(Scancode::HENKAN),
            // KeyCode::KATAKANAHIRAGANA => Some(Scancode::KATAKANAHIRAGANA),
            // KeyCode::MUHENKAN => Some(Scancode::MUHENKAN),
            // KeyCode::KPJPCOMMA => Some(Scancode::KPJPCOMMA),
            // KeyCode::KPEnter => Some(Scancode::KPEnter),
            KeyCode::ControlRight => Some(Scancode::CtrlRight),
            // KeyCode::KPSlash => Some(Scancode::KPSlash),
            // KeyCode::SYSRQ => Some(Scancode::SYSRQ),
            KeyCode::AltRight => Some(Scancode::AltRight),
            // KeyCode::LINEFEED => Some(Scancode::LINEFEED),
            KeyCode::Home => Some(Scancode::Home),
            KeyCode::ArrowUp => Some(Scancode::ArrowUp),
            KeyCode::PageUp => Some(Scancode::PageUp),
            KeyCode::ArrowLeft => Some(Scancode::ArrowLeft),
            KeyCode::ArrowRight => Some(Scancode::ArrowRight),
            KeyCode::End => Some(Scancode::End),
            KeyCode::ArrowDown => Some(Scancode::ArrowDown),
            KeyCode::PageDown => Some(Scancode::PageDown),
            KeyCode::Insert => Some(Scancode::Insert),
            KeyCode::Delete => Some(Scancode::Delete),
            // KeyCode::MACRO => Some(Scancode::MACRO),
            // KeyCode::MUTE => Some(Scancode::MUTE),
            // KeyCode::VOLUMEDOWN => Some(Scancode::VOLUMEDOWN),
            // KeyCode::VOLUMEUP => Some(Scancode::VOLUMEUP),
            // KeyCode::POWER => Some(Scancode::POWER),
            // KeyCode::KPEqual => Some(Scancode::KPEqual),
            // KeyCode::KPPLUSMINUS => Some(Scancode::KPPLUSMINUS),
            // KeyCode::PAUSE => Some(Scancode::PAUSE),
            // KeyCode::SCALE => Some(Scancode::SCALE),
            _ => None,
        },
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
                // NOTE: sdl, wayland provide positions in logical pixels. i kind of want to
                // conform to that across the board.
                let scale_factor = window.scale_factor();
                let position = (position.x / scale_factor, position.y / scale_factor);
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
                    let keycode = match event.logical_key.to_text() {
                        Some(str) if str.chars().count() == 1 => {
                            Keycode::Char(str.chars().next().unwrap())
                        }
                        _ => Keycode::Unhandled,
                    };
                    let pressed = event.state.is_pressed();
                    Some(Event::Keyboard(if pressed {
                        KeyboardEvent::Press {
                            scancode,
                            keycode,
                            repeat: event.repeat,
                        }
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
        Ok(Self {
            event_loop: winit::event_loop::EventLoop::new()?,
            app: App {
                window_attrs: attrs,

                window: None,
                window_create_error: None,

                events: VecDeque::new(),
            },
        })
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
        // NOTE: passing timeout to appear to pump_app_events appear to do absolutely nothing
        // (tested only on wayland). thus control flow "hack"?
        //
        // TODO: support timeout arg that would allow to set different control flows.
        use winit::event_loop::ControlFlow;
        self.event_loop.set_control_flow(ControlFlow::Poll);

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
