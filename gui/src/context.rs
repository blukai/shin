use std::hash::{DefaultHasher, Hash, Hasher};
use std::mem;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use input::{Button, CursorShape};
use nohash::NoHash;

use crate::{
    DrawBuffer, Externs, F64Vec2, FontHandle, FontService, Rect, Rgba, TextureService, Vec2,
};

const DEFAULT_FONT_DATA: &[u8] = include_bytes!("../fixtures/JetBrainsMono-Regular.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key(u64);

impl Key {
    pub fn new<T: Hash>(hashable: T) -> Self {
        // TODO: i probably should not rely on default hasher here?
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

pub struct InteractionRequest {
    key: Key,
    rect: Rect,
    hot_cursor_shape: Option<CursorShape>,
    active_cursor_shape: Option<CursorShape>,
}

impl InteractionRequest {
    pub fn new(key: Key, rect: Rect) -> Self {
        Self {
            key,
            rect,
            hot_cursor_shape: None,
            active_cursor_shape: None,
        }
    }

    pub fn with_cursor_shape(mut self, cursor_shape: CursorShape) -> Self {
        self.hot_cursor_shape = Some(cursor_shape);
        self.active_cursor_shape = Some(cursor_shape);
        self
    }
}

// NOTE: on interactivity (hot, active) watch https://www.youtube.com/watch?v=Z1qyvQsjK5Y.
#[derive(Default)]
pub struct InteractionState {
    /// about to be interacting with this item
    hot: Option<Key>,
    /// items can only become active if they were hot last frame and clicked this frame
    hot_last_frame: Option<Key>,
    /// actually interacting with this item
    active: Option<(Key, Rect)>,

    // NOTE: ui thing has no direct relationship/connection with the windowing system - those would
    // need to consume cursor shape at the end of the ui frame.
    cursor_shape: Option<CursorShape>,
}

impl InteractionState {
    fn begin_iteration(&mut self, input: &input::State) {
        self.hot_last_frame = self.hot.take();
        // NOTE: blur on press, not on release.
        // we don't want to blur active element that maybe is handling text selection (it is normal
        // for text selection to keep updating even if pointer is dragging outside of the widget's
        // bounds) when button is released outside.
        if input.pointer.buttons.just_pressed(Button::Primary)
            && matches!(self.active, Some((_, active_rect)) if {
                    assert!(active_rect.is_normalized());
                    let pointer_position = Vec2::from(F64Vec2::from(input.pointer.position));
                    !active_rect.contains(pointer_position)
            })
        {
            self.active = None;
        }

        // NOTE: start each frame with a default cursor so that the event loop can restore it to
        // default if nothing wants to set it.
        self.cursor_shape = Some(CursorShape::Default);
    }

    fn end_iteration(&mut self) {
        // NOTE: start next frame with a default cursor so that the event loop can restore it to
        // default if nothing wants to set it.
        //
        // NOTE: cursor shape needs to be taken before end of the frame.
        assert!(self.cursor_shape.is_none());
    }

    /// returns true if hit-test succedded.
    pub fn maybe_interact(&mut self, params: InteractionRequest, input: &input::State) -> bool {
        let InteractionRequest {
            key,
            rect,
            hot_cursor_shape,
            active_cursor_shape,
        } = params;
        let pointer_position = Vec2::from(F64Vec2::from(input.pointer.position));

        if !rect.is_normalized() || !rect.contains(pointer_position) {
            return false;
        }

        self.hot = Some(key);
        self.cursor_shape = hot_cursor_shape;

        if input.pointer.buttons.just_pressed(Button::Primary) && self.hot_last_frame == Some(key) {
            self.active = Some((key, rect));
            self.cursor_shape = active_cursor_shape;
        }

        return true;
    }

    pub fn is_hot(&self, key: Key) -> bool {
        self.hot == Some(key)
    }

    pub fn is_active(&self, key: Key) -> bool {
        matches!(self.active, Some((active_key, _)) if active_key == key)
    }

    pub fn set_cursor_shape(&mut self, cursor_shape: CursorShape) {
        self.cursor_shape = Some(cursor_shape);
    }

    pub fn take_cursor_shape(&mut self) -> Option<CursorShape> {
        self.cursor_shape.take()
    }
}

struct ClipboardRead {
    iteration_key: Key,
    request_key: Key,
    payload: Option<anyhow::Result<String>>,
}

struct ClipboardWrite {
    iteration_key: Key,
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
    iteration_key: Option<Key>,
    read: Option<ClipboardRead>,
    write: Option<ClipboardWrite>,
}

impl ClipboardState {
    fn take_iteration_key(&mut self) -> Key {
        self.iteration_key.take().expect("didn't begin frame")
    }

    fn iteration_key(&self) -> Key {
        self.iteration_key.expect("didn't begin frame")
    }

    fn begin_iteration(&mut self, iteration_key: Key) {
        let prev_iteration_key = self.iteration_key.replace(iteration_key);
        assert!(prev_iteration_key.is_none());
    }

    fn end_iteration(&mut self) {
        let iteration_key = self.take_iteration_key();

        // NOTE: clean up request older than current frame (orphaned or unconsumed).
        self.read
            .take_if(|r| r.iteration_key != iteration_key)
            .inspect(|r| log::debug!("[clipboard] evict read (key {:?})", r.request_key));

        // NOTE: clean up request older than current frame (unconsumed).
        self.write
            .take_if(|w| w.iteration_key != iteration_key)
            .inspect(|_| log::debug!("[clipboard] evict write"));
    }

    /// widget requests clipboard read.
    ///
    /// it will only be possible to consume clipboard read next frame.
    pub fn request_read(&mut self, request_key: Key) {
        log::debug!("[clipboard] request read (key {request_key:?})");
        let iteration_key = self.iteration_key();
        self.read = Some(ClipboardRead {
            iteration_key,
            request_key,
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
            .take_if(|r| r.request_key == key && r.payload.is_some())
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
        let iteration_key = self.iteration_key();
        self.write = Some(ClipboardWrite {
            iteration_key,
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

    pub fg: Rgba,
    pub selection_active_bg: Rgba,
    pub selection_inactive_bg: Rgba,
    pub cursor_bg: Rgba,

    pub scroll_delta_factor: Vec2,
}

impl Appearance {
    pub fn new_dark(font_handle: FontHandle) -> Self {
        Self {
            font_handle,
            font_size: 16.0,

            fg: Rgba::WHITE,
            selection_active_bg: Rgba::from_u32(0x304a3dff),
            selection_inactive_bg: Rgba::from_u32(0x484848ff),
            cursor_bg: Rgba::from_u32(0x8faf9fff),

            // NOTE: this is the same as in firefox (in about:config look for
            // mousewheel.default.delta_multiplier_*).
            scroll_delta_factor: Vec2::splat(100.0),
        }
    }
}

// TODO: would be cool to support some kind of render targets or something for viewports to make it
// possible to render multiple ones onto a single surface?

// TODO: how context is supposed to be shared between different surfaces?
// see https://stackoverflow.com/questions/29617370/multiple-opengl-contexts-multiple-windows-multithreading-and-vsync

pub struct Viewport<E: Externs> {
    pub physical_size: Vec2,
    pub scale_factor: f32,

    pub draw_buffer: DrawBuffer<E>,

    previous_frame_start: Instant,
    current_frame_start: Instant,
    // TODO: consider storing delta_time as_secs_f32 because ViewportContext::dt is the only thing
    // that accesses it and it always exposes it as_secs_f32; the computation is not absolutely
    // free.
    delta_time: Duration,

    touched_this_iteration: bool,
}

// @BlindDerive
impl<E: Externs> Default for Viewport<E> {
    fn default() -> Self {
        Self {
            physical_size: Vec2::ZERO,
            scale_factor: 0.0,
            draw_buffer: DrawBuffer::default(),

            // TODO: unfuck instants. they shouldn't be constructed at now.
            previous_frame_start: Instant::now(),
            current_frame_start: Instant::now(),
            delta_time: Duration::ZERO,

            touched_this_iteration: false,
        }
    }
}

impl<E: Externs> Viewport<E> {
    pub fn begin_frame(&mut self, physical_size: Vec2, scale_factor: f32) {
        assert!(physical_size.x > 0.0 && physical_size.y > 0.0);
        assert!(scale_factor > 0.0);

        self.physical_size = physical_size;
        self.scale_factor = scale_factor;

        self.current_frame_start = Instant::now();
        self.delta_time = self.current_frame_start - self.previous_frame_start;
        self.previous_frame_start = self.current_frame_start;

        self.touched_this_iteration = true;
    }

    pub fn end_frame(&mut self) {
        // TODO: rename draw_buffer's clear to end frame. but also make so that renderer drains it,
        // not just gets the data from it.
        self.draw_buffer.clear();

        // NOTE: reset for the next iteration.
        // TODO: this resent probably must not happen at the end of the frame, but at the beginning
        // of the iteration?
        let touched_this_iteration = mem::replace(&mut self.touched_this_iteration, false);
        assert!(touched_this_iteration);
    }

    // TODO: consider removing dt method and instead storing dt as secs f32 and making it public.
    pub fn dt(&self) -> f32 {
        self.delta_time.as_secs_f32()
    }
}

pub struct Context<E: Externs> {
    iteration_num: u64,

    pub interaction_state: InteractionState,
    pub clipboard_state: ClipboardState,
    pub appearance: Appearance,

    pub texture_service: TextureService<E>,
    pub font_service: FontService,
}

impl<E: Externs> Default for Context<E> {
    fn default() -> Self {
        // NOTE: am i okay with paniching here because the panic may only be caused by an invalid
        // font file; you can guarantee valitidy of by not putting an invalid default font into
        // fixtures directory xd.
        Self::new_with_default_font_slice(DEFAULT_FONT_DATA)
            .expect("somebody fucked things up; default font is invalid?")
    }
}

impl<E: Externs> Context<E> {
    pub fn new_with_default_font_slice(default_font_data: &'static [u8]) -> anyhow::Result<Self> {
        let mut font_service = FontService::default();
        let default_font_handle = font_service
            .register_font_slice(default_font_data)
            .context("could not register font slice")?;

        Ok(Self {
            iteration_num: 0,

            interaction_state: InteractionState::default(),
            clipboard_state: ClipboardState::default(),
            appearance: Appearance::new_dark(default_font_handle),

            texture_service: TextureService::default(),
            font_service,
        })
    }

    pub fn begin_iteration(&mut self, input: &input::State) {
        // will overflow in several billion years of running non stop; or in several thousand years
        // in worst(/best) case scenarion of running at thousands frames per sec xd.
        self.iteration_num += 1;
        let iteration_key = Key::from_caller_location_and(self.iteration_num);

        self.interaction_state.begin_iteration(input);
        self.clipboard_state.begin_iteration(iteration_key);

        self.font_service.begin_iteration();
    }

    pub fn end_iteration(&mut self) {
        self.interaction_state.end_iteration();
        self.clipboard_state.end_iteration();

        self.font_service.end_iteration(&mut self.texture_service);
    }
}
