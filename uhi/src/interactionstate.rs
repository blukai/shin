use std::hash::{DefaultHasher, Hash, Hasher};

use input::{CursorShape, PointerButton};

use crate::{F64Vec2, Rect, Vec2};

// NOTE: on interactivity (hot, active) watch https://www.youtube.com/watch?v=Z1qyvQsjK5Y.

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

#[derive(Debug, Default)]
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

    pub fn take_cursor_shape(&mut self) -> Option<CursorShape> {
        self.cursor_shape.take()
    }
}
