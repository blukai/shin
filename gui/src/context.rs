use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::{Duration, Instant};

use input::{Button, CursorShape};
use nohash::NoHash;

use crate::{
    DrawBuffer, Externs, F64Vec2, FontHandle, FontService, Rect, Rgba8, TextureService, Vec2,
};

const DEFAULT_FONT_DATA: &[u8] = include_bytes!("../fixtures/JetBrainsMono-Regular.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key(u64);

impl Key {
    pub fn new<T: Hash>(hashable: T) -> Self {
        let mut hasher = DefaultHasher::default();
        hashable.hash(&mut hasher);
        Self(hasher.finish())
    }

    pub fn from_location(location: &'static std::panic::Location) -> Self {
        Self::new(location)
    }

    #[track_caller]
    pub fn from_caller_location() -> Self {
        Self::new(std::panic::Location::caller())
    }

    /// usefule for when you're rendering something in a loop (your *_and* might be an index).
    #[track_caller]
    pub fn from_caller_location_and<T: Hash>(hashable: T) -> Self {
        Self::new((std::panic::Location::caller(), hashable))
    }
}

impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0);
    }
}

impl NoHash for Key {}

// NOTE: on interactivity (hot, active) watch https://www.youtube.com/watch?v=Z1qyvQsjK5Y.
#[derive(Default)]
pub struct InteractionState {
    /// about to be interacting with this item
    hot: Option<Key>,
    /// items can only become active if they were hot last frame and clicked this frame
    hot_last_frame: Option<Key>,
    /// actually interacting with this item
    active: Option<Key>,

    // NOTE: ui thing has no direct relationship/connection with the windowing system - those would
    // need to consume cursor shape at the end of the ui frame.
    cursor_shape: Option<CursorShape>,
}

impl InteractionState {
    pub fn begin_frame(&mut self) {
        // NOTE: start each frame with a default cursor so that the event loop can restore it to
        // default if nothing wants to set it.
        self.cursor_shape = Some(CursorShape::Default);
    }

    pub fn end_frame(&mut self) {
        self.hot_last_frame = self.hot.take();
        // NOTE: cursor shape needs to be taken before end of the frame.
        assert!(self.cursor_shape.is_none());
    }

    /// returns `true` if element just became active.
    pub fn maybe_set_hot_or_active(
        &mut self,
        key: Key,
        rect: Rect,
        cursor_shape: CursorShape,
        input: &input::State,
    ) -> bool {
        let mut ret = false;

        let inside = rect.contains(Vec2::from(F64Vec2::from(input.pointer.position)));

        // TODO: setting thing inactive on press (not on release) seem too feel more natural, but i
        // am not completely sure yet.
        //
        // NOTE: setting thing inactive on release makes things weird with for example text
        // selection.
        if self.active == Some(key)
            && input.pointer.buttons.just_pressed(Button::Primary)
            && !inside
        {
            self.active = None;
        }

        if self.hot_last_frame == Some(key)
            && input.pointer.buttons.just_pressed(Button::Primary)
            && inside
        {
            self.active = Some(key);
            self.cursor_shape = Some(cursor_shape);
            ret = true;
        }

        if inside {
            self.hot = Some(key);
            self.cursor_shape = Some(cursor_shape);
        }

        ret
    }

    pub fn is_hot(&self, key: Key) -> bool {
        self.hot == Some(key)
    }

    pub fn is_active(&self, key: Key) -> bool {
        self.active == Some(key)
    }

    pub fn take_cursor_shape(&mut self) -> Option<CursorShape> {
        self.cursor_shape.take()
    }
}

struct ClipboardRead {
    key: Key,
    frame_time: Instant,
    payload: Option<anyhow::Result<String>>,
}

struct ClipboardWrite {
    frame_time: Instant,
    payload: String,
}

/// reads are lagged 1 frame behind:
/// - ui requests a read at frame 1;
/// - event loop fulfills it at the end of frame 1;
/// - ui may consume the read at frame 2.
///
/// writes are immediate:
/// - ui requests a write at frame 1;
/// - event loop fulfills it at the end of frame 1.
#[derive(Default)]
pub struct ClipboardState {
    current_frame_start: Option<Instant>,
    read: Option<ClipboardRead>,
    write: Option<ClipboardWrite>,
}

impl ClipboardState {
    pub fn begin_frame(&mut self, current_frame_start: Instant) {
        self.current_frame_start = Some(current_frame_start);
    }

    pub fn end_frame(&mut self) {
        let current_frame_start = self.current_frame_start.take().expect("didn't begin frame");

        // NOTE: clean up request older than current frame (orphaned or unconsumed).
        self.read
            .take_if(|r| r.frame_time < current_frame_start)
            .inspect(|cr| log::debug!("[clipboard] evict read (key {:?})", cr.key));

        // NOTE: clean up request older than current frame (unconsumed).
        self.write
            .take_if(|w| w.frame_time < current_frame_start)
            .inspect(|_| log::debug!("[clipboard] evict write"));
    }

    /// widget requests clipboard read.
    ///
    /// it will only be possible to consume clipboard read next frame.
    pub fn request_read(&mut self, key: Key) {
        log::debug!("[clipboard] request read (key {key:?})");
        let frame_time = self.current_frame_start.expect("didn't begin frame");
        self.read = Some(ClipboardRead {
            key,
            frame_time,
            payload: None,
        });
    }

    /// event loop needs to fulfill clipboard read request at the end of the frame at which the
    /// read was requested so that widget(the requester) can take(/consume) the result next frame.
    pub fn is_awaiting_read(&mut self) -> bool {
        self.read.as_ref().is_some_and(|r| r.payload.is_none())
    }

    /// [`Self::fulfill_read`] must be called only if [`Self::is_awaiting_read`] returned true.
    pub fn fulfill_read(&mut self, payload: anyhow::Result<String>) {
        assert!(self.is_awaiting_read());
        let Some(ref mut r) = self.read else {
            unreachable!();
        };
        r.payload = Some(payload);
    }

    /// widget(/the requester) takes(/consumes) clipboard read.
    pub fn try_take_read(&mut self, key: Key) -> Option<String> {
        self.read
            .take_if(|r| r.key == key && r.payload.is_some())
            .and_then(|r| match r.payload {
                Some(Ok(payload)) => {
                    log::debug!("[clipboard] took successful read ({key:?})");
                    Some(payload)
                }
                Some(Err(err)) => {
                    // TODO: do i need to be somehow more elaborate with handling this error? this
                    // semi-silent approach is probably ok.
                    log::error!("[clipboard] took (but sort of ignored) failed read: {err:?}");
                    None
                }
                None => unreachable!(),
            })
    }

    /// widget requests clipboard write.
    pub fn request_write(&mut self, payload: String) {
        log::debug!("[clipboard] request write (text {payload})");
        let frame_time = self.current_frame_start.expect("didn't begin frame");
        self.write = Some(ClipboardWrite {
            frame_time,
            payload,
        });
    }

    /// event loop needs to put this into clipboard at the end of the frame.
    pub fn take_write(&mut self) -> Option<String> {
        self.write.take().map(|w| w.payload)
    }
}

#[derive(Clone)]
pub struct Appearance {
    pub font_handle: FontHandle,
    pub font_size: f32,

    pub fg: Rgba8,
    pub selection_active_bg: Rgba8,
    pub selection_inactive_bg: Rgba8,
    pub cursor_bg: Rgba8,

    pub scroll_delta_factor: Vec2,
}

impl Appearance {
    pub fn new_dark(font_handle: FontHandle) -> Self {
        Self {
            font_handle,
            font_size: 16.0,

            fg: Rgba8::WHITE,
            selection_active_bg: Rgba8::from_u32(0x304a3dff),
            selection_inactive_bg: Rgba8::from_u32(0x484848ff),
            cursor_bg: Rgba8::from_u32(0x8faf9fff),

            // NOTE: this is the same as in firefox (in about:config look for
            // mousewheel.default.delta_multiplier_*).
            scroll_delta_factor: Vec2::splat(100.0),
        }
    }
}

// TODO: consider introducing viewports or something like that.
// scale factor, refresh rate, etc. may vary per monitor thus each viewport probably will need to
// own its timings, and possibly interaction state?

// TODO: would be cool to support some kind of render targets or something for viewports to make it
// possible to render multiple ones onto a single surface?

// TODO: begin_frame and end_frame stuff is bad.
// when working with multiple surfaces a single "pass" will consist of mutlple "frames" on
// different "surfaces".

pub struct Context<E: Externs> {
    pub scale_factor: f32,

    previous_frame_start: Instant,
    current_frame_start: Instant,
    delta_time: Duration,

    pub font_service: FontService,
    pub texture_service: TextureService<E>,
    pub draw_buffer: DrawBuffer<E>,

    pub interaction_state: InteractionState,
    pub clipboard_state: ClipboardState,

    pub appearance: Appearance,
}

impl<E: Externs> Default for Context<E> {
    fn default() -> Self {
        Self::new_with_default_font_slice(DEFAULT_FONT_DATA)
            .expect("somebody fucked things up; default font is invalid?")
    }
}

impl<E: Externs> Context<E> {
    pub fn new_with_default_font_slice(default_font_data: &'static [u8]) -> anyhow::Result<Self> {
        let mut font_service = FontService::default();
        let default_font_handle = font_service.register_font_slice(default_font_data)?;

        Ok(Self {
            scale_factor: 1.0,

            previous_frame_start: Instant::now(),
            current_frame_start: Instant::now(),
            delta_time: Duration::ZERO,

            texture_service: TextureService::default(),
            font_service,
            draw_buffer: DrawBuffer::default(),

            interaction_state: InteractionState::default(),
            clipboard_state: ClipboardState::default(),

            appearance: Appearance::new_dark(default_font_handle),
        })
    }

    pub fn begin_frame(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor;

        self.current_frame_start = Instant::now();
        self.delta_time = self.current_frame_start - self.previous_frame_start;
        self.previous_frame_start = self.current_frame_start;

        self.font_service.begin_frame();

        self.interaction_state.begin_frame();
        self.clipboard_state.begin_frame(self.current_frame_start);
    }

    pub fn end_frame(&mut self) {
        self.font_service.end_frame(&mut self.texture_service);
        // TODO: rename draw_buffer's clear to end frame. but also make so that renderer drains it,
        // not just gets the data from it.
        self.draw_buffer.clear();

        self.interaction_state.end_frame();
        self.clipboard_state.end_frame();
    }

    // TODO: consider removing dt method and instead storing dt as secs f32 and making it public.
    pub fn dt(&self) -> f32 {
        self.delta_time.as_secs_f32()
    }
}
