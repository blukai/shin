use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::{Duration, Instant};

use input::{CursorShape, PointerButton};

use crate::{DrawBuffer, Externs, F64Vec2, FontHandle, FontService, Rect, TextureService, Vec2};

const DEFAULT_FONT_DATA: &[u8] = include_bytes!("../fixtures/JetBrainsMono-Regular.ttf");

// NOTE: on interactivity (hot, active) watch https://www.youtube.com/watch?v=Z1qyvQsjK5Y.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key(u64);

impl Key {
    pub fn new<T: Hash>(hashable: T) -> Self {
        let mut hasher = DefaultHasher::default();
        hashable.hash(&mut hasher);
        Self(hasher.finish())
    }

    #[track_caller]
    pub fn from_location() -> Self {
        Self::new(std::panic::Location::caller())
    }
}

pub struct Context<E: Externs> {
    pub font_service: FontService,
    pub texture_service: TextureService<E>,
    pub draw_buffer: DrawBuffer<E>,

    default_font_handle: FontHandle,
    default_font_size: f32,

    previous_frame_start: Instant,
    current_frame_start: Instant,
    delta_time: Duration,

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

impl<E: Externs> Default for Context<E> {
    fn default() -> Self {
        Self::with_default_font_slice(DEFAULT_FONT_DATA, 16.0)
            .expect("somebody fucked things up; default font is invalid?")
    }
}

impl<E: Externs> Context<E> {
    pub fn with_default_font_slice(
        font_data: &'static [u8],
        default_font_size: f32,
    ) -> anyhow::Result<Self> {
        let mut font_service = FontService::default();
        let default_font_handle = font_service.register_font_slice(font_data)?;

        Ok(Self {
            texture_service: TextureService::default(),
            font_service,
            draw_buffer: DrawBuffer::default(),

            default_font_handle,
            default_font_size,

            previous_frame_start: Instant::now(),
            current_frame_start: Instant::now(),
            delta_time: Duration::ZERO,

            hot: None,
            hot_last_frame: None,
            active: None,

            cursor_shape: None,
        })
    }
    pub fn begin_frame(&mut self) {
        self.current_frame_start = Instant::now();
        self.delta_time = self.current_frame_start - self.previous_frame_start;
        self.previous_frame_start = self.current_frame_start;
    }

    pub fn end_frame(&mut self) {
        self.draw_buffer.clear();

        self.hot_last_frame = self.hot.take();

        self.cursor_shape = None;
    }

    pub fn default_font_handle(&self) -> FontHandle {
        self.default_font_handle
    }

    pub fn default_font_size(&self) -> f32 {
        self.default_font_size
    }

    pub fn dt(&self) -> f32 {
        self.delta_time.as_secs_f32()
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

        let inside = rect.contains(&Vec2::from(F64Vec2::from(input.pointer.position)));

        // TODO: setting thing inactive on press (not on release) seem too feel more natural, but i
        // am not completely sure yet.
        //
        // NOTE: setting thing inactive on release makes things weird with for example text
        // selection.
        if self.active == Some(key)
            && input.pointer.buttons.just_pressed(PointerButton::Primary)
            && !inside
        {
            self.active = None;
        }

        if self.hot_last_frame == Some(key)
            && input.pointer.buttons.just_pressed(PointerButton::Primary)
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

    // TODO: consider maybe somehow hinting/emphasising that this needs to be "consumed" at the end
    // of each frame?
    pub fn cursor_shape(&self) -> Option<CursorShape> {
        self.cursor_shape
    }
}
