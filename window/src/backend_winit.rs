use std::collections::VecDeque;

use anyhow::{Context, anyhow};
use input::{
    Button, ButtonState, CursorShape, KeyState, KeyboardEvent, Keycode, PointerEvent, RawKey,
    Scancode,
};
use raw_window_handle as rwh;
use winit::platform::pump_events::EventLoopExtPumpEvents;

use crate::{ClipboardDataProvider, DEFAULT_LOGICAL_SIZE, Event, Window, WindowAttrs, WindowEvent};

#[inline]
fn map_element_state_to_button_state(element_state: winit::event::ElementState) -> ButtonState {
    use winit::event::ElementState;
    match element_state {
        ElementState::Pressed => ButtonState::Pressed,
        ElementState::Released => ButtonState::Released,
    }
}

#[inline]
fn map_element_state_to_key_state(element_state: winit::event::ElementState) -> KeyState {
    use winit::event::ElementState;
    match element_state {
        ElementState::Pressed => KeyState::Pressed,
        ElementState::Released => KeyState::Released,
    }
}

#[inline]
fn try_map_pointer_button(button: winit::event::MouseButton) -> Option<Button> {
    use winit::event::MouseButton;
    match button {
        MouseButton::Left => Some(Button::Primary),
        MouseButton::Right => Some(Button::Secondary),
        MouseButton::Middle => Some(Button::Tertiary),
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
        CursorShape::Crosshair => CursorIcon::Crosshair,
        CursorShape::Move => CursorIcon::Move,
        CursorShape::NwResize => CursorIcon::NwResize,
        CursorShape::NeResize => CursorIcon::NeResize,
        CursorShape::SeResize => CursorIcon::SeResize,
        CursorShape::SwResize => CursorIcon::SwResize,
    })
}

#[inline]
fn map_keyboard_physical_key(physical_key: winit::keyboard::PhysicalKey) -> Scancode {
    use winit::keyboard::{KeyCode, PhysicalKey};
    match physical_key {
        PhysicalKey::Code(keycode) => match keycode {
            // KeyCode::Reserved => Scancode::Reserved,
            KeyCode::Escape => Scancode::Esc,
            KeyCode::Digit1 => Scancode::Num1,
            KeyCode::Digit2 => Scancode::Num2,
            KeyCode::Digit3 => Scancode::Num3,
            KeyCode::Digit4 => Scancode::Num4,
            KeyCode::Digit5 => Scancode::Num5,
            KeyCode::Digit6 => Scancode::Num6,
            KeyCode::Digit7 => Scancode::Num7,
            KeyCode::Digit8 => Scancode::Num8,
            KeyCode::Digit9 => Scancode::Num9,
            KeyCode::Digit0 => Scancode::Num0,
            KeyCode::Minus => Scancode::Minus,
            KeyCode::Equal => Scancode::Equal,
            KeyCode::Backspace => Scancode::Backspace,
            KeyCode::Tab => Scancode::Tab,
            KeyCode::KeyQ => Scancode::Q,
            KeyCode::KeyW => Scancode::W,
            KeyCode::KeyE => Scancode::E,
            KeyCode::KeyR => Scancode::R,
            KeyCode::KeyT => Scancode::T,
            KeyCode::KeyY => Scancode::Y,
            KeyCode::KeyU => Scancode::U,
            KeyCode::KeyI => Scancode::I,
            KeyCode::KeyO => Scancode::O,
            KeyCode::KeyP => Scancode::P,
            KeyCode::BracketLeft => Scancode::BraceLeft,
            KeyCode::BracketRight => Scancode::BraceRight,
            KeyCode::Enter => Scancode::Enter,
            KeyCode::ControlLeft => Scancode::CtrlLeft,
            KeyCode::KeyA => Scancode::A,
            KeyCode::KeyS => Scancode::S,
            KeyCode::KeyD => Scancode::D,
            KeyCode::KeyF => Scancode::F,
            KeyCode::KeyG => Scancode::G,
            KeyCode::KeyH => Scancode::H,
            KeyCode::KeyJ => Scancode::J,
            KeyCode::KeyK => Scancode::K,
            KeyCode::KeyL => Scancode::L,
            KeyCode::Semicolon => Scancode::Semicolon,
            KeyCode::Quote => Scancode::Apostrophe,
            KeyCode::Backquote => Scancode::Grave,
            KeyCode::ShiftLeft => Scancode::ShiftLeft,
            KeyCode::Backslash => Scancode::Backslash,
            KeyCode::KeyZ => Scancode::Z,
            KeyCode::KeyX => Scancode::X,
            KeyCode::KeyC => Scancode::C,
            KeyCode::KeyV => Scancode::V,
            KeyCode::KeyB => Scancode::B,
            KeyCode::KeyN => Scancode::N,
            KeyCode::KeyM => Scancode::M,
            KeyCode::Comma => Scancode::Comma,
            KeyCode::Period => Scancode::Dot,
            KeyCode::Slash => Scancode::Slash,
            KeyCode::ShiftRight => Scancode::ShiftRight,
            // KeyCode::KPAsterisk => Scancode::KPAsterisk,
            KeyCode::AltLeft => Scancode::AltLeft,
            KeyCode::Space => Scancode::Space,
            KeyCode::CapsLock => Scancode::CapsLock,
            // KeyCode::F1 => Scancode::F1,
            // KeyCode::F2 => Scancode::F2,
            // KeyCode::F3 => Scancode::F3,
            // KeyCode::F4 => Scancode::F4,
            // KeyCode::F5 => Scancode::F5,
            // KeyCode::F6 => Scancode::F6,
            // KeyCode::F7 => Scancode::F7,
            // KeyCode::F8 => Scancode::F8,
            // KeyCode::F9 => Scancode::F9,
            // KeyCode::F10 => Scancode::F10,
            KeyCode::NumLock => Scancode::NumLock,
            KeyCode::ScrollLock => Scancode::ScrollLock,
            // KeyCode::KP7 => Scancode::KP7,
            // KeyCode::KP8 => Scancode::KP8,
            // KeyCode::KP9 => Scancode::KP9,
            // KeyCode::KPMinus => Scancode::KPMinus,
            // KeyCode::KP4 => Scancode::KP4,
            // KeyCode::KP5 => Scancode::KP5,
            // KeyCode::KP6 => Scancode::KP6,
            // KeyCode::KPPlus => Scancode::KPPlus,
            // KeyCode::KP1 => Scancode::KP1,
            // KeyCode::KP2 => Scancode::KP2,
            // KeyCode::KP3 => Scancode::KP3,
            // KeyCode::KP0 => Scancode::KP0,
            // KeyCode::KPDOT => Scancode::KPDOT,
            // KeyCode::_ => Scancode::_,
            // KeyCode::ZENKAKUHANKAKU => Scancode::ZENKAKUHANKAKU,
            // KeyCode::102ND => Scancode::102ND,
            // KeyCode::F11 => Scancode::F11,
            // KeyCode::F12 => Scancode::F12,
            // KeyCode::RO => Scancode::RO,
            // KeyCode::KATAKANA => Scancode::KATAKANA,
            // KeyCode::HIRAGANA => Scancode::HIRAGANA,
            // KeyCode::HENKAN => Scancode::HENKAN,
            // KeyCode::KATAKANAHIRAGANA => Scancode::KATAKANAHIRAGANA,
            // KeyCode::MUHENKAN => Scancode::MUHENKAN,
            // KeyCode::KPJPCOMMA => Scancode::KPJPCOMMA,
            // KeyCode::KPEnter => Scancode::KPEnter,
            KeyCode::ControlRight => Scancode::CtrlRight,
            // KeyCode::KPSlash => Scancode::KPSlash,
            // KeyCode::SYSRQ => Scancode::SYSRQ,
            KeyCode::AltRight => Scancode::AltRight,
            // KeyCode::LINEFEED => Scancode::LINEFEED,
            KeyCode::Home => Scancode::Home,
            KeyCode::ArrowUp => Scancode::ArrowUp,
            KeyCode::PageUp => Scancode::PageUp,
            KeyCode::ArrowLeft => Scancode::ArrowLeft,
            KeyCode::ArrowRight => Scancode::ArrowRight,
            KeyCode::End => Scancode::End,
            KeyCode::ArrowDown => Scancode::ArrowDown,
            KeyCode::PageDown => Scancode::PageDown,
            KeyCode::Insert => Scancode::Insert,
            KeyCode::Delete => Scancode::Delete,
            // KeyCode::MACRO => Scancode::MACRO,
            // KeyCode::MUTE => Scancode::MUTE,
            // KeyCode::VOLUMEDOWN => Scancode::VOLUMEDOWN,
            // KeyCode::VOLUMEUP => Scancode::VOLUMEUP,
            // KeyCode::POWER => Scancode::POWER,
            // KeyCode::KPEqual => Scancode::KPEqual,
            // KeyCode::KPPLUSMINUS => Scancode::KPPLUSMINUS,
            // KeyCode::PAUSE => Scancode::PAUSE,
            // KeyCode::SCALE => Scancode::SCALE,
            _ => Scancode::Unidentified(RawKey::Unidentified),
        },
        // TODO: maybe map NativeKeyCode::Xkb to RawKey::Unix ?
        _ => Scancode::Unidentified(RawKey::Unidentified),
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
            CursorEntered { .. } => Some(Event::Pointer(PointerEvent::Enter { position: None })),
            CursorLeft { .. } => Some(Event::Pointer(PointerEvent::Leave)),
            CursorMoved { position, .. } => {
                // NOTE: sdl, wayland provide positions in logical pixels. i kind of want to
                // conform to that across the board.
                let scale_factor = window.scale_factor();
                let position = (position.x / scale_factor, position.y / scale_factor);
                Some(Event::Pointer(PointerEvent::Move { position }))
            }
            MouseInput { button, state, .. } => {
                if let Some(button) = try_map_pointer_button(button) {
                    let state = map_element_state_to_button_state(state);
                    Some(Event::Pointer(PointerEvent::Button { state, button }))
                } else {
                    None
                }
            }
            KeyboardInput { event, .. } => {
                let scancode = map_keyboard_physical_key(event.physical_key);
                let state = map_element_state_to_key_state(event.state);
                let keycode = match event.logical_key.to_text() {
                    Some(str) if str.chars().count() == 1 => {
                        Keycode::Char(str.chars().next().unwrap())
                    }
                    // TODO: maybe map NativeKeyCode::Xkb to RawKey::Unix ?
                    _ => Keycode::Unidentified(RawKey::Unidentified),
                };
                Some(Event::Keyboard(KeyboardEvent::Key {
                    state,
                    scancode,
                    keycode,
                    repeat: event.repeat,
                }))
            }
            MouseWheel {
                delta: mouse_scroll_delta,
                ..
            } => {
                use winit::event::MouseScrollDelta;
                let delta = match mouse_scroll_delta {
                    MouseScrollDelta::LineDelta(x, y) => (x as f64, y as f64),
                    MouseScrollDelta::PixelDelta(physical_position) => {
                        use raw_window_handle::HasWindowHandle as _;
                        match window.window_handle().map(|wh| wh.as_raw()) {
                            Ok(rwh::RawWindowHandle::Wayland(_)) => {
                                // NOTE: on wayland winit does not do anything with wl_pointer_axis values,
                                // which is great. we can normanize them the same way we do in wayland
                                // backend (in wayland pointer frame handling code look for comments
                                // surrounding axis handling).
                                const SCALE: f64 = 10.0;
                                (physical_position.x / SCALE, physical_position.y / SCALE)
                            }
                            _ => (physical_position.x, physical_position.y),
                        }
                    }
                };
                Some(Event::Pointer(PointerEvent::Scroll {
                    // NOTE: winit inverts deltas.
                    delta: (-delta.0, -delta.1),
                }))
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

    fn read_clipboard(&mut self, _mime_type: &str, _buf: &mut Vec<u8>) -> anyhow::Result<usize> {
        log::warn!("winit backend does not support clipboard");
        // TODO: support wayland clipboard (but first separate it out from wayland backend).
        Ok(0)
    }

    fn provide_clipboard_data(
        &mut self,
        _data_provider: Box<dyn ClipboardDataProvider>,
    ) -> anyhow::Result<()> {
        log::warn!("winit backend does not support clipboard");
        // TODO: support wayland clipboard (but first separate it out from wayland backend).
        Ok(())
    }

    fn physical_size(&self) -> (u32, u32) {
        let window = self.app.window.as_ref().expect("initialized window");
        let inner_physical_size = window.inner_size();
        (inner_physical_size.width, inner_physical_size.height)
    }

    fn scale_factor(&self) -> f64 {
        let window = self.app.window.as_ref().expect("initialized window");
        window.scale_factor()
    }
}
