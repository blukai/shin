use std::{ops::Range, panic::Location};

use input::{Keycode, Scancode};

use crate::{
    Context, Externs, Fill, FillTexture, FontHandle, Key, Rect, RectShape, Rgba8, TextureKind, Vec2,
};

// TODO: multiline / singleline distinction
// - both likely need to know max width
// - multiline needs to know max height
// - both need to be vertically and horizontally scrollable
//
// TODO: per-char layout styling
// - should be able to make some fragments of text bold?
// - should be able to change some elements of appearance (fg, bg)
//
// TODO: filters / input types
// - for example number-only input, etc.
//
// TODO: color schemes ?
//
// TODO: clarity separation between:
// - not selectable and not editable (no state and no input)
// - selectable but not editable (needs state and input)
// - selectable and editable (needs state and input)

const FG: Rgba8 = Rgba8::WHITE;
const CURSOR: Rgba8 = Rgba8::from_u32(0x8faf9fff);
const SELECTION_ACTIVE: Rgba8 = Rgba8::from_u32(0x304a3dff);
const SELECTION_INACTIVE: Rgba8 = Rgba8::from_u32(0x484848ff);

pub struct TextAppearance {
    pub font_handle: FontHandle,
    pub font_size: f32,

    pub fg: Option<Rgba8>,
    pub cursor_active: Option<Rgba8>,
    pub cursor_inactive: Option<Rgba8>,
    pub selection_active: Option<Rgba8>,
    pub selection_inactive: Option<Rgba8>,
    // pub container_bg: Option<Rgba8>,
    // pub container_stroke: Option<Rgba8>,
    // pub container_padding: Option<Vec2>,
}

impl TextAppearance {
    pub fn new(font_handle: FontHandle, font_size: f32) -> Self {
        Self {
            font_handle,
            font_size,

            fg: None,
            cursor_active: None,
            cursor_inactive: None,
            selection_active: None,
            selection_inactive: None,
        }
    }

    pub fn fg(mut self, fg: Rgba8) -> Self {
        self.fg = Some(fg);
        self
    }

    // TODO: more builder methods
}

// ----

pub struct TextState {
    key: Key,
    // if equal, no selection; start may be less than or greater than end (start is where the
    // initial click was).
    cursor: Range<usize>,
}

impl Default for TextState {
    #[track_caller]
    fn default() -> Self {
        Self {
            key: Key::new(Location::caller()),
            cursor: Default::default(),
        }
    }
}

impl TextState {
    pub fn key(mut self, value: Key) -> Self {
        self.key = value;
        self
    }

    // ----

    fn has_selection(&self) -> bool {
        self.cursor.start != self.cursor.end
    }

    fn normalized_selection(&self) -> Range<usize> {
        let left = self.cursor.start.min(self.cursor.end);
        let right = self.cursor.start.max(self.cursor.end);
        left..right
    }

    // TODO: move modifiers (by char, by char type, by word, etc.)

    fn move_cursor_left(&mut self, text: &str, extend_selection: bool) {
        if self.has_selection() && !extend_selection {
            self.cursor.end = self.cursor.end.min(self.cursor.start);
            self.cursor.start = self.cursor.end;
            return;
        }

        let prev_char_width = &text[..self.cursor.end]
            .chars()
            .next_back()
            .map_or_else(|| 0, |ch| ch.len_utf8());
        self.cursor.end -= prev_char_width;
        if !extend_selection {
            self.cursor.start = self.cursor.end;
        }
    }

    fn move_cursor_right(&mut self, text: &str, extend_selection: bool) {
        if self.has_selection() && !extend_selection {
            self.cursor.end = self.cursor.end.max(self.cursor.start);
            self.cursor.start = self.cursor.end;
            return;
        }

        let next_char_width = &text[self.cursor.end..]
            .chars()
            .next()
            .map_or_else(|| 0, |ch| ch.len_utf8());
        self.cursor.end += next_char_width;
        if !extend_selection {
            self.cursor.start = self.cursor.end;
        }
    }

    fn delete_selection(&mut self, text: &mut String) {
        let range = self.normalized_selection();
        if range.end > range.start {
            text.replace_range(range, "");
        }
        self.cursor.end = self.cursor.end.min(self.cursor.start);
        self.cursor.start = self.cursor.end;
    }

    fn delete_left(&mut self, text: &mut String) {
        if !self.has_selection() {
            self.cursor.end = self.cursor.start;
            self.move_cursor_left(text, true);
        }
        self.delete_selection(text);
    }

    fn delete_right(&mut self, text: &mut String) {
        if !self.has_selection() {
            self.cursor.end = self.cursor.start;
            self.move_cursor_right(text, true);
        }
        self.delete_selection(text);
    }

    fn insert_char(&mut self, text: &mut String, ch: char) {
        if self.has_selection() {
            self.delete_selection(text);
        }
        assert_eq!(self.cursor.start, self.cursor.end);
        text.insert(self.cursor.start, ch);
        self.cursor.start += ch.len_utf8();
        self.cursor.end = self.cursor.start;
    }

    // TODO: mouse selection (sometking like drag or drag_start and drag end?)
}

// ----

pub enum TextKind<'a> {
    Readonly(&'a str),
    Editable(&'a mut String),
}

impl<'a> TextKind<'a> {
    #[inline]
    fn as_str(&self) -> &str {
        match self {
            Self::Readonly(s) => s,
            Self::Editable(s) => s.as_str(),
        }
    }

    #[inline]
    fn as_string_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::Readonly(_) => None,
            Self::Editable(s) => Some(s),
        }
    }
}

// ----

pub fn draw_text<E: Externs>(
    mut text: TextKind,
    // NOTE: if state is None - text is not interactable.
    mut state: Option<&mut TextState>,
    appearance: &TextAppearance,
    // TODO: consider replacing position with Placement enum or something?
    // - singleline variant will need an position and width.
    // - multiline variant will need an area rect.
    position: Vec2,
    input: Option<&input::State>,
    ctx: &mut Context<E>,
) {
    let mut font_instance_ref = ctx
        .font_service
        .get_font_instance_mut(appearance.font_handle, appearance.font_size);

    if let (Some(state), Some(input)) = (state.as_deref_mut(), input) {
        // TODO: text rect must take into account container's padding
        let rect = Rect::new(
            position,
            position + font_instance_ref.compute_text_size(text.as_str(), &mut ctx.texture_service),
        );
        ctx.interaction_state
            .maybe_set_hot_or_active(state.key, rect, input);

        // TODO: move state updating (event handling) out
        if ctx.interaction_state.is_active(state.key) {
            // TODO: consider separating selectable-not-mutable and selectable-and-mutable state
            // updates

            let scancodes = &input.keyboard.scancodes;
            let keycodes = &input.keyboard.keycodes;

            if scancodes.just_pressed(Scancode::ArrowLeft) {
                state.move_cursor_left(
                    text.as_str(),
                    scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]),
                );
            }
            if scancodes.just_pressed(Scancode::ArrowRight) {
                state.move_cursor_right(
                    text.as_str(),
                    scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]),
                );
            }

            if let Some(text) = text.as_string_mut() {
                if scancodes.just_pressed(Scancode::Backspace) {
                    state.delete_left(text);
                }
                if scancodes.just_pressed(Scancode::Delete) {
                    state.delete_right(text);
                }
                for keycode in keycodes.iter_just_pressed() {
                    match keycode {
                        Keycode::Char(ch) => {
                            let printable = (ch as u32 >= 32) && (ch as u32 != 127);
                            if printable {
                                state.insert_char(text, ch);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // ----

    let text = text.as_str();

    if let Some(state) = state {
        let cursor_end_x = font_instance_ref
            .compute_text_width(&text[..state.cursor.end], &mut ctx.texture_service);

        if state.has_selection() {
            let cursor_start_x = font_instance_ref
                .compute_text_width(&text[..state.cursor.start], &mut ctx.texture_service);
            let left = cursor_start_x.min(cursor_end_x);
            let right = cursor_start_x.max(cursor_end_x);
            let selection_rect = {
                let min = position + Vec2::new(left, 0.0);
                let size = Vec2::new(right - left, font_instance_ref.line_height());
                Rect::new(min, min + size)
            };

            ctx.draw_buffer.push_rect(RectShape::with_fill(
                selection_rect,
                Fill::with_color(if ctx.interaction_state.is_active(state.key) {
                    SELECTION_ACTIVE
                } else {
                    SELECTION_INACTIVE
                }),
            ));
        }

        if ctx.interaction_state.is_active(state.key) {
            let cursor_rect = {
                let cursor_width: f32 = font_instance_ref.typical_advance_width();
                let min = position + Vec2::new(cursor_end_x, 0.0);
                let size = Vec2::new(cursor_width, font_instance_ref.line_height());
                Rect::new(min, min + size)
            };

            ctx.draw_buffer
                .push_rect(RectShape::with_fill(cursor_rect, Fill::with_color(CURSOR)));
        }
    }

    let fg = appearance.fg.unwrap_or(FG);
    let mut offset_x = position.x;
    let mut offset_y = position.y;
    let ascent = font_instance_ref.ascent();
    for ch in text.chars() {
        // TODO: multiline wrapping (max width)
        // if ch == '\n' {
        //     offset_x = position.x;
        //     offset_y += font_instance_ref.line_height();
        //     continue;
        // }

        let char_ref = font_instance_ref.get_char(ch, &mut ctx.texture_service);
        let char_bounds = char_ref.bounds();

        ctx.draw_buffer.push_rect(RectShape::with_fill(
            char_bounds.translate_by(&Vec2::new(offset_x, offset_y + ascent)),
            Fill::new(
                fg,
                FillTexture {
                    kind: TextureKind::Internal(char_ref.tex_handle()),
                    coords: char_ref.tex_coords(),
                },
            ),
        ));
        offset_x += char_ref.advance_width();
    }
}

pub fn draw_readonly_text<E: Externs>(
    text: &str,
    appearance: &TextAppearance,
    position: Vec2,
    ctx: &mut Context<E>,
) {
    draw_text(
        TextKind::Readonly(text),
        None,
        appearance,
        position,
        None,
        ctx,
    )
}

pub fn draw_editable_text<E: Externs>(
    text: &mut String,
    state: &mut TextState,
    appearance: &TextAppearance,
    position: Vec2,
    input: &input::State,
    ctx: &mut Context<E>,
) {
    draw_text(
        TextKind::Editable(text),
        Some(state),
        appearance,
        position,
        Some(input),
        ctx,
    )
}
