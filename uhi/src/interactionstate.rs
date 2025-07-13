use std::hash::{DefaultHasher, Hash, Hasher};

use input::PointerButton;

use crate::{F64Vec2, Rect, Vec2};

// watch https://www.youtube.com/watch?v=Z1qyvQsjK5Y.

// NOTE: widgets must generate id with in a state "constructor" which must have the #[track_caller]
// attribute; also widgets must allow to specify id manually (because when rendering lists
// location-based id would dup in a loop).
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

#[derive(Debug, Default)]
pub struct InteractionState {
    /// about to be interacting with this item
    hot_key: Option<Key>,
    /// items can only become active if they were hot last frame and clicked this frame
    hot_key_last_frame: Option<Key>,
    /// actually interacting with this item
    active_key: Option<Key>,
}

impl InteractionState {
    pub fn begin_frame(&mut self) {
        self.hot_key = None;
    }

    pub fn end_frame(&mut self) {
        self.hot_key_last_frame = self.hot_key;
    }

    pub fn maybe_set_hot_or_active(&mut self, key: Key, rect: Rect, input: &input::State) {
        let inside = rect.contains(&Vec2::from(F64Vec2::from(input.pointer.position)));

        // TODO: consider setting things inactive on press (not on release). doing that on press
        // seem too feel more natural, but i am not completely sure yet...
        if self.active_key == Some(key)
            && input.pointer.buttons.just_released(PointerButton::Primary)
            && !inside
        {
            self.active_key = None;
        }

        if self.hot_key_last_frame == Some(key)
            && input.pointer.buttons.just_pressed(PointerButton::Primary)
            && inside
        {
            self.active_key = Some(key);
        }

        if inside {
            self.hot_key = Some(key);
        }
    }

    pub fn is_hot(&self, key: Key) -> bool {
        self.hot_key == Some(key)
    }

    pub fn is_active(&self, key: Key) -> bool {
        self.active_key == Some(key)
    }
}
