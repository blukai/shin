use std::hash::Hash;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerButton {
    /// equivalent to left mouse button
    Primary = 1,
    /// equivalent to right mouse button
    Secondary = 1 << 2,
    /// equivalent to middle mouse button
    Tertiary = 1 << 3,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PointerButtons(u8);

impl PointerButtons {
    pub fn contains(&self, which: PointerButton) -> bool {
        self.0 & which as u8 == which as u8
    }

    pub(crate) fn set(&mut self, which: PointerButton, value: bool) {
        if value {
            self.0 |= which as u8
        } else {
            self.0 &= !(which as u8);
        }
    }
}

#[test]
fn test_pointer_buttons_set() {
    let mut pointer_buttons = PointerButtons::default();

    pointer_buttons.set(PointerButton::Primary, true);
    assert!(pointer_buttons.contains(PointerButton::Primary));

    pointer_buttons.set(PointerButton::Secondary, true);
    assert!(pointer_buttons.contains(PointerButton::Primary));
    assert!(pointer_buttons.contains(PointerButton::Secondary));

    pointer_buttons.set(PointerButton::Primary, false);
    assert!(!pointer_buttons.contains(PointerButton::Primary));
}

#[derive(Debug)]
pub enum PointerEventKind {
    Motion { delta: (f32, f32) },
    Press { button: PointerButton },
    Release { button: PointerButton },
}

#[derive(Debug)]
pub struct PointerEvent {
    pub kind: PointerEventKind,
    pub position: (f32, f32),
    pub buttons: PointerButtons,
}
