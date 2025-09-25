use std::hash::{Hash, Hasher};

use nohash::{NoHash, NoHashMap};

// TODO: events must carry device id in addition to surface id.
// (on device id) maybe you want to let people play split screen with with different controllers
// (event though i am absolutely clueless and never did own one).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceId(pub u64);

// pointer
// ----

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// equivalent to left mouse button
    Primary,
    /// equivalent to right mouse button
    Secondary,
    /// equivalent to middle mouse button
    Tertiary,
}

impl Hash for Button {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(*self as u8);
    }
}

impl NoHash for Button {}

impl Button {
    /// NOTE: this is useful for calling InputState's all_just_pressed/all_just_released method for
    /// example.
    pub fn all() -> [Self; 3] {
        use Button::*;
        [Primary, Secondary, Tertiary]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Pressed,
    Released,
}

// NOTE: refs on how platforms handle gestures:
// - https://wayland.app/protocols/pointer-gestures-unstable-v1
// - https://developer.apple.com/documentation/appkit/nsgesturerecognizer
//   - https://developer.apple.com/documentation/uikit/uigesturerecognizer
// - https://learn.microsoft.com/en-us/windows/win32/wintouch/windows-touch-gestures-overview
// - https://developer.android.com/develop/ui/compose/touch-input/pointer-input/multi-touch
// and libs:
// - https://doc.qt.io/qt-6/qt.html#GestureType-enum
//   - https://doc.qt.io/qt-6/qtwidgets-gestures-imagegestures-example.html
//   - https://doc.qt.io/qt-6/qpinchgesture.html

// NOTE: pan, zoom, rotate - all can be dispatched simulateneously.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GesturePhase {
    Started,
    Updated,
    Finished,
    Cancelled,
}

// TODO: PointerEventKind is awkward because it does not carry position info for events other then
// Move. PointerState has it, but there's absolutely no guaranttee that it's in sync. InputState
// always represents the latest, but use may be processing events one by one and when click happens
// position in InputState may not exactly-correctly represent click's position.
//
// TODO: consider implementing Swipe event. on wayland perhaps you can listen for hold event with 2
// fingers followed by horizontal scroll?
#[derive(Debug, Clone)]
pub enum PointerEventKind {
    Enter {
        // NOTE: winit (v 0.30.12) does not provide enter position. but it seems like future
        // versions will.
        position: Option<(f64, f64)>,
    },
    Leave,
    Move {
        position: (f64, f64),
    },
    Button {
        state: ButtonState,
        button: Button,
    },
    // TODO: should scroll event provide more data? currently i normalize delta in wayland backend,
    // and i repeat that in winit backend if winit is running under wayland...
    //
    // should scroll event provide pixel (pixel delta in probably physical surface space), should
    // it provide discrete delta (which is in steps)?
    Scroll {
        // TODO: consider being more descriptive with what delta this is (like gesture events).
        delta: (f64, f64),
    },
    // TODO: winit does not support gestures (only on ios?). extract gesture handling from wayland
    // backend and use it in winit backend if winint backend is using wayland under the hood.
    Pan {
        phase: GesturePhase,
        translation_delta: (f64, f64),
        /// on wayland might be 2 if pan is triggered pinch, might be 3 if triggered by swipe.
        touches: u8,
    },
    Zoom {
        phase: GesturePhase,
        /// scale relative to the initial finger position
        scale_delta: f64,
    },
    Rotate {
        phase: GesturePhase,
        /// angle in degrees cw relative to the previous event
        rotation_delta: f64,
    },
}

#[derive(Debug, Clone)]
pub struct PointerEvent {
    pub surface_id: SurfaceId,
    pub kind: PointerEventKind,
}

// https://developer.mozilla.org/en-US/docs/Web/CSS/cursor
// https://github.com/manu-mannattil/adwaita-cursors/blob/9e9929fa544985623574df3e214ec03299baa251/Makefile
// $ find /usr/share/icons/Adwaita/cursors -type f
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Default,
    Pointer,
    Text,
    Crosshair,
    Move,
    Grab,
    Grabbing,
    ColResize,
    EResize,
    EwResize,
    NResize,
    NeResize,
    NeswResize,
    NsResize,
    NwResize,
    NwseResize,
    RowResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
}

// keyboard
// ----

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawKey {
    /// linux [1], and pretty sure would work just fine on freebsd and its forks [2], but not mac.
    ///
    /// [1]: https://github.com/torvalds/linux/blob/8d561baae505bab6b3f133e10dc48e27e4505cbe/include/uapi/linux/input-event-codes.h
    /// [2]: https://github.com/freebsd/freebsd-src/blob/18a870751b036f1dc78b36084ccb993d139a11bb/sys/dev/evdev/input-event-codes.h
    Unix(u32),
    Unidentified,
}

/// Scancode is a hardware-generated code that corresponds to the physical key pressed on the
/// keyboard. It represents the physical location of the key regardless of the keyboard layout.
/// Scancodes are consistent across different keyboard layouts and are generated by the keyboard
/// hardware itself. For example, pressing the key in the position of "Q" on a QWERTY keyboard will
/// generate the same scancode even if the keyboard layout is AZERTY or Dvorak
///   - llm
///
/// https://github.com/torvalds/linux/blob/231825b2e1ff6ba799c5eaf396d3ab2354e37c6b/include/uapi/linux/input-event-codes.h#L76
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Scancode {
    Reserved,               // KEY_RESERVED          0
    Esc,                    // KEY_ESC               1
    Num1,                   // KEY_1                 2
    Num2,                   // KEY_2                 3
    Num3,                   // KEY_3                 4
    Num4,                   // KEY_4                 5
    Num5,                   // KEY_5                 6
    Num6,                   // KEY_6                 7
    Num7,                   // KEY_7                 8
    Num8,                   // KEY_8                 9
    Num9,                   // KEY_9                 10
    Num0,                   // KEY_0                 11
    Minus,                  // KEY_MINUS             12
    Equal,                  // KEY_EQUAL             13
    Backspace,              // KEY_BACKSPACE         14
    Tab,                    // KEY_TAB               15
    Q,                      // KEY_Q                 16
    W,                      // KEY_W                 17
    E,                      // KEY_E                 18
    R,                      // KEY_R                 19
    T,                      // KEY_T                 20
    Y,                      // KEY_Y                 21
    U,                      // KEY_U                 22
    I,                      // KEY_I                 23
    O,                      // KEY_O                 24
    P,                      // KEY_P                 25
    BraceLeft,              // KEY_LEFTBRACE         26 /* [ */
    BraceRight,             // KEY_RIGHTBRACE        27 /* ] */
    Enter,                  // KEY_ENTER             28
    CtrlLeft,               // KEY_LEFTCTRL          29
    A,                      // KEY_A                 30
    S,                      // KEY_S                 31
    D,                      // KEY_D                 32
    F,                      // KEY_F                 33
    G,                      // KEY_G                 34
    H,                      // KEY_H                 35
    J,                      // KEY_J                 36
    K,                      // KEY_K                 37
    L,                      // KEY_L                 38
    Semicolon,              // KEY_SEMICOLON         39
    Apostrophe,             // KEY_APOSTROPHE        40
    Grave,                  // KEY_GRAVE             41
    ShiftLeft,              // KEY_LEFTSHIFT         42
    Backslash,              // KEY_BACKSLASH         43
    Z,                      // KEY_Z                 44
    X,                      // KEY_X                 45
    C,                      // KEY_C                 46
    V,                      // KEY_V                 47
    B,                      // KEY_B                 48
    N,                      // KEY_N                 49
    M,                      // KEY_M                 50
    Comma,                  // KEY_COMMA             51
    Dot,                    // KEY_DOT               52
    Slash,                  // KEY_SLASH             53
    ShiftRight,             // KEY_RIGHTSHIFT        54
    // KPAsterisk,          // KEY_KPASTERISK        55
    AltLeft,                // KEY_LEFTALT           56
    Space,                  // KEY_SPACE             57
    CapsLock,               // KEY_CAPSLOCK          58
    // F1,                  // KEY_F1                59
    // F2,                  // KEY_F2                60
    // F3,                  // KEY_F3                61
    // F4,                  // KEY_F4                62
    // F5,                  // KEY_F5                63
    // F6,                  // KEY_F6                64
    // F7,                  // KEY_F7                65
    // F8,                  // KEY_F8                66
    // F9,                  // KEY_F9                67
    // F10,                 // KEY_F10               68
    NumLock,                // KEY_NUMLOCK           69
    ScrollLock,             // KEY_SCROLLLOCK        70
    // KP7,                 // KEY_KP7               71
    // KP8,                 // KEY_KP8               72
    // KP9,                 // KEY_KP9               73
    // KPMinus,             // KEY_KPMINUS           74
    // KP4,                 // KEY_KP4               75
    // KP5,                 // KEY_KP5               76
    // KP6,                 // KEY_KP6               77
    // KPPlus,              // KEY_KPPLUS            78
    // KP1,                 // KEY_KP1               79
    // KP2,                 // KEY_KP2               80
    // KP3,                 // KEY_KP3               81
    // KP0,                 // KEY_KP0               82
    // KPDOT,               // KEY_KPDOT             83
    // _,
    // ZENKAKUHANKAKU,      // KEY_ZENKAKUHANKAKU    85
    // 102ND,               // KEY_102ND             86
    // F11,                 // KEY_F11               87
    // F12,                 // KEY_F12               88
    // RO,                  // KEY_RO                89
    // KATAKANA,            // KEY_KATAKANA          90
    // HIRAGANA,            // KEY_HIRAGANA          91
    // HENKAN,              // KEY_HENKAN            92
    // KATAKANAHIRAGANA,    // KEY_KATAKANAHIRAGANA  93
    // MUHENKAN,            // KEY_MUHENKAN          94
    // KPJPCOMMA,           // KEY_KPJPCOMMA         95
    // KPEnter,             // KEY_KPENTER           96
    CtrlRight,              // KEY_RIGHTCTRL         97
    // KPSlash,             // KEY_KPSLASH           98
    // SYSRQ,               // KEY_SYSRQ             99
    AltRight,               // KEY_RIGHTALT          100
    // LINEFEED,            // KEY_LINEFEED          101
    Home,                   // KEY_HOME              102
    ArrowUp,                // KEY_UP                103
    PageUp,                 // KEY_PAGEUP            104
    ArrowLeft,              // KEY_LEFT              105
    ArrowRight,             // KEY_RIGHT             106
    End,                    // KEY_END               107
    ArrowDown,              // KEY_DOWN              108
    PageDown,               // KEY_PAGEDOWN          109
    Insert,                 // KEY_INSERT            110
    Delete,                 // KEY_DELETE            111
    // MACRO,               // KEY_MACRO             112
    // MUTE,                // KEY_MUTE              113
    // VOLUMEDOWN,          // KEY_VOLUMEDOWN        114
    // VOLUMEUP,            // KEY_VOLUMEUP          115
    // POWER,               // KEY_POWER             116 /* SC System Power Down */
    // KPEqual,             // KEY_KPEQUAL           117
    // KPPLUSMINUS,         // KEY_KPPLUSMINUS       118
    // PAUSE,               // KEY_PAUSE             119
    // SCALE,               // KEY_SCALE             120 /* AL Compiz Scale (Expose) */
    Unidentified(RawKey),
}

impl Hash for Scancode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Unidentified(unidentified) => {
                // NOTE: the idea here is to kind of "seed" hashes for unidentified keys.
                state.write_u32(u32::MAX);

                match unidentified {
                    RawKey::Unix(scancode) => state.write_u32(*scancode),
                    RawKey::Unidentified => {
                        // NOTE: ^ we ~seeded the hash above. there's nothing more to do really.
                    }
                }
            }
            identified => {
                // SAFETY: Because `Self` is marked `repr(u32)`, its layout is a `repr(C)` `union`
                // between `repr(C)` structs, each of which has the `u32` discriminant as its first
                // field, so we can read the discriminant without offsetting the pointer.
                //
                // NOTE: this is based on <https://doc.rust-lang.org/std/mem/fn.discriminant.html>.
                let discriminant = unsafe { *<*const _>::from(identified).cast::<u32>() };
                state.write_u32(discriminant);
            }
        }
    }
}

impl NoHash for Scancode {}

/// Keycode is a code assigned by the operating system or software that represents the symbol or
/// character mapped to the key pressed, taking into account the current keyboard layout. For
/// example, pressing the same physical key might generate a different keycode on an AZERTY
/// keyboard compared to a QWERTY keyboard because the symbol mapped to that key differs
///   - llm
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keycode {
    Char(char),
    Unidentified(RawKey),
    // TODO: consider mapping scancode to keycode somehow to respect keyboard layouts. and maybe
    // don't operate on scancodes at all?
}

impl Hash for Keycode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Char(ch) => state.write_u32(*ch as u32),
            Self::Unidentified(unidentified) => {
                // NOTE: the idea here is to kind of "seed" hashes for unidentified keys.
                //
                // according to https://www.rfc-editor.org/rfc/rfc3629 In UTF-8, characters from
                // the U+0000..U+10FFFF. 0x10FFFF = 1114111 (21 bits)
                state.write_u32(u32::MAX);

                match unidentified {
                    RawKey::Unix(scancode) => state.write_u32(*scancode),
                    RawKey::Unidentified => {
                        // NOTE: ^ we ~seeded the hash above. there's nothing more to do really.
                    }
                }
            }
        }
    }
}

impl NoHash for Keycode {}

// TODO: consider converting KeyboardEventKind::Key's repeat bool into KeyState::Repeated variant
// (this will match WL_KEYBOARD_KEY_STATE_* enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

// TODO: combine Press and Release events into Button event with
// enum ButtonState { Pressed, Released }
#[derive(Debug, Clone)]
pub enum KeyboardEventKind {
    Key {
        state: KeyState,
        scancode: Scancode,
        keycode: Keycode,
        /// true if this is a key repeat
        repeat: bool,
    },
}

#[derive(Debug, Clone)]
pub struct KeyboardEvent {
    pub surface_id: SurfaceId,
    pub kind: KeyboardEventKind,
}

// states
// ----

// TODO: might want to implement bitwise op traits for StateFlags.
//
// NOTE: button may have multiple states at the same time.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StateFlags(u8);

impl StateFlags {
    pub const NONE: u8 = 0;
    pub const JUST_PRESSED: u8 = 1 << 0;
    pub const JUST_RELEASED: u8 = 1 << 1;
    pub const DOWN: u8 = 1 << 2;
    pub const REPEAT: u8 = 1 << 3;
}

// NOTE: this was originally inspired by bevy's ButtonInput thing.
#[derive(Debug)]
pub struct StateTracker<B>
where
    B: Copy + Eq + NoHash,
{
    map: NoHashMap<B, StateFlags>,
}

// @BlindDerive
impl<B> Default for StateTracker<B>
where
    B: Copy + Eq + NoHash,
{
    fn default() -> Self {
        Self {
            map: NoHashMap::default(),
        }
    }
}

impl<B> StateTracker<B>
where
    B: Copy + Eq + NoHash,
{
    pub fn clear_transient_flags(&mut self) {
        self.map.values_mut().for_each(|state| {
            state.0 &= !StateFlags::JUST_PRESSED;
            state.0 &= !StateFlags::JUST_RELEASED;
            state.0 &= !StateFlags::REPEAT;
        });
    }

    // ----

    pub fn press(&mut self, button: B, repeat: bool) {
        let state = self.map.entry(button).or_insert(StateFlags(0));
        state.0 = StateFlags::JUST_PRESSED | StateFlags::DOWN;
        if repeat {
            state.0 |= StateFlags::REPEAT
        }
    }

    pub fn release(&mut self, button: B) {
        let state = self.map.entry(button).or_insert(StateFlags(0));
        state.0 = StateFlags::JUST_RELEASED;
    }

    // just pressed

    pub fn just_pressed(&self, button: B) -> bool {
        self.map
            .get(&button)
            .is_some_and(|state| state.0 & StateFlags::JUST_PRESSED != 0)
    }

    pub fn any_just_pressed(&self, buttons: impl IntoIterator<Item = B>) -> bool {
        buttons.into_iter().any(|button| self.just_pressed(button))
    }

    pub fn all_just_pressed(&self, buttons: impl IntoIterator<Item = B>) -> bool {
        buttons.into_iter().all(|button| self.just_pressed(button))
    }

    pub fn iter_just_pressed(&self) -> impl Iterator<Item = B> {
        self.map.iter().filter_map(|(button, state)| {
            (state.0 & StateFlags::JUST_PRESSED != 0).then_some(*button)
        })
    }

    // just released

    pub fn just_released(&self, button: B) -> bool {
        self.map
            .get(&button)
            .is_some_and(|state| state.0 & StateFlags::JUST_RELEASED != 0)
    }

    pub fn any_just_released(&self, buttons: impl IntoIterator<Item = B>) -> bool {
        buttons.into_iter().any(|button| self.just_released(button))
    }

    pub fn all_just_released(&self, buttons: impl IntoIterator<Item = B>) -> bool {
        buttons.into_iter().all(|button| self.just_released(button))
    }

    pub fn iter_just_released(&self) -> impl Iterator<Item = B> {
        self.map.iter().filter_map(|(button, state)| {
            (state.0 & StateFlags::JUST_RELEASED != 0).then_some(*button)
        })
    }

    // down

    pub fn down(&self, button: B) -> bool {
        self.map
            .get(&button)
            .is_some_and(|state| state.0 & StateFlags::DOWN != 0)
    }

    pub fn any_down(&self, buttons: impl IntoIterator<Item = B>) -> bool {
        buttons.into_iter().any(|button| self.down(button))
    }

    pub fn all_down(&self, buttons: impl IntoIterator<Item = B>) -> bool {
        buttons.into_iter().all(|button| self.down(button))
    }

    pub fn iter_down(&self) -> impl Iterator<Item = B> {
        self.map
            .iter()
            .filter_map(|(button, state)| (state.0 & StateFlags::DOWN != 0).then_some(*button))
    }

    // repeat

    pub fn repeated(&self, button: B) -> bool {
        self.map
            .get(&button)
            .is_some_and(|state| state.0 & StateFlags::REPEAT != 0)
    }

    pub fn any_repeated(&self, buttons: impl IntoIterator<Item = B>) -> bool {
        buttons.into_iter().any(|button| self.repeated(button))
    }

    pub fn all_repeated(&self, buttons: impl IntoIterator<Item = B>) -> bool {
        buttons.into_iter().all(|button| self.repeated(button))
    }

    pub fn iter_repeated(&self) -> impl Iterator<Item = B> {
        self.map
            .iter()
            .filter_map(|(button, state)| (state.0 & StateFlags::REPEAT != 0).then_some(*button))
    }
}

#[derive(Debug, Default)]
pub struct PointerState {
    pub position: Option<(f64, f64)>,
    // NOTE: prev_position is needed to compute position_delta.
    //   a single iteration (of an event loop) may accumulate multiple move events thus to compute
    //   correct deltas we need to diff against prev frame and not against prev value.
    prev_position: Option<(f64, f64)>,
    pub position_delta: Option<(f64, f64)>,

    // NOTE: scroll_delta is a accumulator that is being reset each iteration.
    //   accumulator because multiple scroll events may be received per iteration(/frame).
    pub scroll_delta: Option<(f64, f64)>,

    pub buttons: StateTracker<Button>,
    pub press_origins: NoHashMap<Button, (f64, f64)>,
}

impl PointerState {
    #[inline]
    pub fn reset_deltas(&mut self) {
        self.prev_position = self.position;
        self.position_delta = None;

        self.scroll_delta = None;
    }

    #[inline]
    pub fn clear_transient_flags(&mut self) {
        self.buttons.clear_transient_flags();
    }

    #[inline]
    pub fn handle_event(&mut self, ev: PointerEvent) {
        use PointerEventKind::*;
        match ev.kind {
            // NOTE: (on Enter) when window spawns right under the cursor doing this helps to
            // compute correct deltas and dispatch press with correct delta.
            Enter {
                position: Some(position),
            }
            | Move { position } => {
                self.position = Some(position);
                if let Some(prev) = self.prev_position {
                    let delta = (position.0 - prev.0, position.1 - prev.1);
                    if delta != (0.0, 0.0) {
                        self.position_delta = Some(delta);
                    }
                }
            }
            Leave => {
                // TODO: would it make sense to clear/reset position and delta values on pointer
                // leave?
            }
            Scroll { delta } => {
                let acc = self.scroll_delta.get_or_insert((0.0, 0.0));
                acc.0 += delta.0;
                acc.1 += delta.1;
            }
            Button {
                state: ButtonState::Pressed,
                button,
            } => {
                self.buttons.press(button, false);
                let position = self
                    .position
                    .expect("Enter or Move must have occured before Button");
                self.press_origins.insert(button, position);
            }
            Button {
                state: ButtonState::Released,
                button,
            } => {
                self.buttons.release(button);
                self.press_origins.remove(&button);
            }
            _ => {}
        }
    }
}

// TODO: might want to implement bitwise op traits for ModifierFlags.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ModifierFlags(u16);

impl ModifierFlags {
    pub const CTRL_LEFT: u16 = 1 << 0;
    pub const CTRL_RIGHT: u16 = 1 << 1;
    pub const SHIFT_LEFT: u16 = 1 << 2;
    pub const SHIFT_RIGHT: u16 = 1 << 3;
    pub const ALT_LEFT: u16 = 1 << 4;
    pub const ALT_RIGHT: u16 = 1 << 5;

    pub const CTRL: u16 = Self::CTRL_LEFT | Self::CTRL_RIGHT;
    pub const SHIFT: u16 = Self::SHIFT_LEFT | Self::SHIFT_RIGHT;
    pub const ALT: u16 = Self::ALT_LEFT | Self::ALT_RIGHT;

    pub fn try_from_scancode(scancode: Scancode) -> Option<Self> {
        match scancode {
            Scancode::CtrlLeft => Some(Self(Self::CTRL_LEFT)),
            Scancode::CtrlRight => Some(Self(Self::CTRL_RIGHT)),
            Scancode::ShiftLeft => Some(Self(Self::SHIFT_LEFT)),
            Scancode::ShiftRight => Some(Self(Self::SHIFT_RIGHT)),
            Scancode::AltLeft => Some(Self(Self::ALT_LEFT)),
            Scancode::AltRight => Some(Self(Self::ALT_RIGHT)),
            _ => None,
        }
    }

    // TODO: would i ever want to look for just either left or right mod but not both?

    pub fn ctrl(&self) -> bool {
        self.0 & Self::CTRL != 0
    }

    pub fn shift(&self) -> bool {
        self.0 & Self::SHIFT != 0
    }

    pub fn alt(&self) -> bool {
        self.0 & Self::ALT != 0
    }
}

#[derive(Debug, Default)]
pub struct KeyboardState {
    pub scancodes: StateTracker<Scancode>,
    pub keycodes: StateTracker<Keycode>,
    pub modifiers: ModifierFlags,
}

impl KeyboardState {
    #[inline]
    pub fn clear_transient_flags(&mut self) {
        self.scancodes.clear_transient_flags();
        self.keycodes.clear_transient_flags();
    }

    #[inline]
    pub fn handle_event(&mut self, ev: KeyboardEvent) {
        use KeyboardEventKind::*;
        match ev.kind {
            Key {
                state: KeyState::Pressed,
                scancode,
                keycode,
                repeat,
            } => {
                self.scancodes.press(scancode, repeat);
                self.keycodes.press(keycode, repeat);
                if let Some(ModifierFlags(flags)) = ModifierFlags::try_from_scancode(scancode) {
                    self.modifiers.0 |= flags;
                }
            }
            Key {
                state: KeyState::Released,
                scancode,
                keycode,
                ..
            } => {
                self.scancodes.release(scancode);
                self.keycodes.release(keycode);
                if let Some(ModifierFlags(flags)) = ModifierFlags::try_from_scancode(scancode) {
                    self.modifiers.0 &= !flags;
                }
            }
        }
    }
}

/// Event is not for you to really use on your side. it's more of an internal thing for this mod.
#[derive(Debug, Clone)]
pub enum Event {
    Pointer(PointerEvent),
    Keyboard(KeyboardEvent),
}

#[derive(Debug, Default)]
pub struct State {
    pub pointer: PointerState,
    pub keyboard: KeyboardState,
    /// event accumulator.
    ///
    /// NOTE: do not rely on `PointerState`/`KeyboardState` while iterating over `events` because
    /// states reflect the latest values while events preserve historical sequence.
    /// for example a press at events[0] may have happened at different position than the current
    /// pointer state's position if subsequent events updated the position.
    pub events: Vec<Event>,
}

impl State {
    pub fn handle_events(&mut self, events: impl Iterator<Item = Event>) {
        self.pointer.reset_deltas();
        self.pointer.clear_transient_flags();
        self.keyboard.clear_transient_flags();
        self.events.clear();

        for event in events {
            match event.clone() {
                Event::Pointer(ev) => self.pointer.handle_event(ev),
                Event::Keyboard(ev) => self.keyboard.handle_event(ev),
            }
            self.events.push(event);
        }
    }
}
