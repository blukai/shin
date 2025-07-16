use std::ops::Range;

use input::{Event, KeyboardEvent, KeyboardState, Keycode, PointerButton, PointerEvent, Scancode};

use crate::{
    Context, Externs, F64Vec2, Fill, FillTexture, FontHandle, Key, Rect, RectShape, Rgba8,
    TextureKind, Vec2,
};

// TODO: vertically and horizontally scrollable editors

// TODO: per-char layout styling
// - should be able to make some fragments of text bold?
// - should be able to change some elements of palette (fg, etc.)

// TODO: filters / input types
// - for example number-only input, etc.

// TODO: color schemes ? consider making TextPalette part of something more "centeralized" in
// combination with other styles? part of Context maybe?

const FG: Rgba8 = Rgba8::WHITE;
const SELECTION_ACTIVE: Rgba8 = Rgba8::from_u32(0x304a3dff);
const SELECTION_INACTIVE: Rgba8 = Rgba8::from_u32(0x484848ff);
const CURSOR: Rgba8 = Rgba8::from_u32(0x8faf9fff);

pub struct TextPalette {
    pub fg: Rgba8,
    pub selection_active: Rgba8,
    pub selection_inactive: Rgba8,
    pub cursor: Rgba8,
}

impl Default for TextPalette {
    fn default() -> Self {
        Self {
            fg: FG,
            selection_active: SELECTION_ACTIVE,
            selection_inactive: SELECTION_INACTIVE,
            cursor: CURSOR,
        }
    }
}

impl TextPalette {
    pub fn with_fg(mut self, value: Rgba8) -> Self {
        self.fg = value;
        self
    }

    pub fn with_selection_active(mut self, value: Rgba8) -> Self {
        self.selection_active = value;
        self
    }

    pub fn with_selection_inactive(mut self, value: Rgba8) -> Self {
        self.selection_inactive = value;
        self
    }

    pub fn with_cursor(mut self, value: Rgba8) -> Self {
        self.cursor = value;
        self
    }
}

#[derive(Default)]
pub struct TextSelection {
    // if equal, no selection; start may be less than or greater than end (start is where the
    // initial click was).
    cursor: Range<usize>,
}

impl TextSelection {
    pub fn is_empty(&self) -> bool {
        self.cursor.start == self.cursor.end
    }

    pub fn clear(&mut self) {
        self.cursor = 0..0;
    }

    // TODO: move modifiers (by char, by char type, by word, etc.)

    fn move_cursor_left(&mut self, text: &str, extend_selection: bool) {
        if !self.is_empty() && !extend_selection {
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
        if !self.is_empty() && !extend_selection {
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

    fn normalized_selection(&self) -> Range<usize> {
        let left = self.cursor.start.min(self.cursor.end);
        let right = self.cursor.start.max(self.cursor.end);
        left..right
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
        if self.is_empty() {
            self.cursor.end = self.cursor.start;
            self.move_cursor_left(text, true);
        }
        self.delete_selection(text);
    }

    fn delete_right(&mut self, text: &mut String) {
        if self.is_empty() {
            self.cursor.end = self.cursor.start;
            self.move_cursor_right(text, true);
        }
        self.delete_selection(text);
    }

    fn insert_char(&mut self, text: &mut String, ch: char) {
        if !self.is_empty() {
            self.delete_selection(text);
        }
        assert_eq!(self.cursor.start, self.cursor.end);
        text.insert(self.cursor.start, ch);
        self.cursor.start += ch.len_utf8();
        self.cursor.end = self.cursor.start;
    }
}

#[non_exhaustive]
pub enum TextBuffer<'a> {
    Str(&'a str),
    StringMut(&'a mut String),
}

impl<'a> TextBuffer<'a> {
    #[inline]
    fn as_str(&self) -> &str {
        match self {
            Self::Str(s) => s,
            Self::StringMut(s) => s.as_str(),
        }
    }

    #[inline]
    fn as_string_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::Str(_) => None,
            Self::StringMut(s) => Some(s),
        }
    }
}

impl<'a> From<&'a str> for TextBuffer<'a> {
    fn from(value: &'a str) -> Self {
        TextBuffer::Str(value)
    }
}

impl<'a> From<&'a mut String> for TextBuffer<'a> {
    fn from(value: &'a mut String) -> Self {
        TextBuffer::StringMut(value)
    }
}

pub struct Text<'a> {
    buffer: TextBuffer<'a>,
    font_handle: FontHandle,
    font_size: f32,
    rect: Rect,
    palette: Option<TextPalette>,
}

impl<'a> Text<'a> {
    pub fn new<B: Into<TextBuffer<'a>>>(
        text: B,
        font_handle: FontHandle,
        font_size: f32,
        rect: Rect,
    ) -> Self {
        Self {
            buffer: text.into(),
            font_handle,
            font_size,
            rect,
            palette: None,
        }
    }

    pub fn with_palette(mut self, value: TextPalette) -> Self {
        self.palette = Some(value);
        self
    }

    pub fn singleline(self) -> TextSingleline<'a> {
        TextSingleline::new(self)
    }
}

// ----

fn compute_singleline_text_size<E: Externs>(text: &Text, ctx: &mut Context<E>) -> Vec2 {
    let mut font_instance_ref = ctx
        .font_service
        .get_font_instance_mut(text.font_handle, text.font_size);
    let width =
        font_instance_ref.compute_text_width(text.buffer.as_str(), &mut ctx.texture_service);
    let height = font_instance_ref.height();
    Vec2::new(width, height)
}

// returns byte offset(not char index)
fn locate_singleline_text_char<E: Externs>(
    text: &Text,
    position: Vec2,
    ctx: &mut Context<E>,
) -> usize {
    // maybe we're dragging and the pointer is before beginning of the line.
    if position.x < text.rect.min.x {
        return 0;
    }

    let mut font_instance_ref = ctx
        .font_service
        .get_font_instance_mut(text.font_handle, text.font_size);

    let mut byte_offset: usize = 0;
    let mut offset_x: f32 = text.rect.min.x;

    for ch in text.buffer.as_str().chars() {
        let char_ref = font_instance_ref.get_char(ch, &mut ctx.texture_service);

        let start_x = offset_x;
        let end_x = start_x + char_ref.advance_width();
        if position.x >= start_x && position.x <= end_x {
            // NOTE: it seems like everyone consider char selected only if you're reaching past
            // half of it.
            let mid_x = start_x + char_ref.advance_width() / 2.0;
            if position.x < mid_x {
                return byte_offset;
            } else {
                return byte_offset + ch.len_utf8();
            }
        }

        byte_offset += ch.len_utf8();
        offset_x += char_ref.advance_width();
    }

    // the pointer is after end of the line.
    assert!(position.x > offset_x);
    text.buffer.as_str().len()
}

fn maybe_draw_singleline_text_selection<E: Externs>(
    text: &Text,
    selection: &TextSelection,
    active: bool,
    ctx: &mut Context<E>,
) {
    if selection.is_empty() {
        return;
    }

    let mut font_instance_ref = ctx
        .font_service
        .get_font_instance_mut(text.font_handle, text.font_size);

    let start_x = font_instance_ref.compute_text_width(
        &text.buffer.as_str()[..selection.cursor.start],
        &mut ctx.texture_service,
    );
    let end_x = font_instance_ref.compute_text_width(
        &text.buffer.as_str()[..selection.cursor.end],
        &mut ctx.texture_service,
    );
    let left = start_x.min(end_x);
    let right = start_x.max(end_x);
    let min = text.rect.min + Vec2::new(left, 0.0);
    let size = Vec2::new(right - left, font_instance_ref.height());

    let selection_rect = Rect::new(min, min + size);
    let selection_fill = if active {
        text.palette
            .as_ref()
            .map(|a| a.selection_active)
            .unwrap_or(SELECTION_ACTIVE)
    } else {
        text.palette
            .as_ref()
            .map(|a| a.selection_inactive)
            .unwrap_or(SELECTION_INACTIVE)
    };
    ctx.draw_buffer.push_rect(RectShape::with_fill(
        selection_rect,
        Fill::with_color(selection_fill),
    ));
}

fn maybe_draw_singleline_text_cursor<E: Externs>(
    text: &Text,
    selection: &TextSelection,
    active: bool,
    ctx: &mut Context<E>,
) {
    if !active {
        return;
    }

    let mut font_instance_ref = ctx
        .font_service
        .get_font_instance_mut(text.font_handle, text.font_size);

    let end_x = font_instance_ref.compute_text_width(
        &text.buffer.as_str()[..selection.cursor.end],
        &mut ctx.texture_service,
    );
    let min = text.rect.min + Vec2::new(end_x, 0.0);
    let width = font_instance_ref.typical_advance_width();
    let size = Vec2::new(width, font_instance_ref.height());

    let cursor_rect = Rect::new(min, min + size);
    let cursor_fill = text.palette.as_ref().map(|a| a.cursor).unwrap_or(CURSOR);
    ctx.draw_buffer.push_rect(RectShape::with_fill(
        cursor_rect,
        Fill::with_color(cursor_fill),
    ));
}

fn draw_singleline_text<E: Externs>(text: &Text, ctx: &mut Context<E>) {
    let mut font_instance_ref = ctx
        .font_service
        .get_font_instance_mut(text.font_handle, text.font_size);
    let ascent = font_instance_ref.ascent();

    let fg = text.palette.as_ref().map(|a| a.fg).unwrap_or(FG);

    let mut offset_x: f32 = text.rect.min.x;
    for ch in text.buffer.as_str().chars() {
        let char_ref = font_instance_ref.get_char(ch, &mut ctx.texture_service);

        ctx.draw_buffer.push_rect(RectShape::with_fill(
            char_ref
                .bounding_rect()
                .translate_by(&Vec2::new(offset_x, text.rect.min.y + ascent)),
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

pub struct TextSingleline<'a> {
    text: Text<'a>,
}

impl<'a> TextSingleline<'a> {
    fn new(text: Text<'a>) -> Self {
        Self { text }
    }

    pub fn draw<E: Externs>(self, ctx: &mut Context<E>) {
        ctx.draw_buffer.set_clip_rect(Some(self.text.rect.clone()));

        draw_singleline_text(&self.text, ctx);

        ctx.draw_buffer.set_clip_rect(None);
    }

    pub fn selectable(self, selection: &'a mut TextSelection) -> TextSinglelineSelectable<'a> {
        TextSinglelineSelectable::new(self.text, selection)
    }

    pub fn editable(self, selection: &'a mut TextSelection) -> TextSinglelineEditable<'a> {
        TextSinglelineEditable::new(self.text, selection)
    }
}

pub struct TextSinglelineSelectable<'a> {
    text: Text<'a>,
    selection: &'a mut TextSelection,

    hot: bool,
    active: bool,
}

impl<'a> TextSinglelineSelectable<'a> {
    fn new(text: Text<'a>, selection: &'a mut TextSelection) -> Self {
        Self {
            text,
            selection,

            hot: false,
            active: false,
        }
    }

    pub fn with_hot(mut self, value: bool) -> Self {
        self.hot = value;
        self
    }

    pub fn with_active(mut self, value: bool) -> Self {
        self.active = value;
        self
    }

    pub fn is_hot(&self) -> bool {
        self.hot
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn maybe_set_hot_or_active<E: Externs>(
        mut self,
        key: Key,
        ctx: &mut Context<E>,
        input: &input::State,
    ) -> Self {
        let size = compute_singleline_text_size(&self.text, ctx);
        let interaction_rect = Rect::new(self.text.rect.min, self.text.rect.min + size);
        ctx.interaction_state
            .maybe_set_hot_or_active(key, interaction_rect, input);
        self.hot = ctx.interaction_state.is_hot(key);
        self.active = ctx.interaction_state.is_active(key);
        self
    }

    pub fn update<E: Externs>(self, ctx: &mut Context<E>, input: &input::State) -> Self {
        let KeyboardState { ref scancodes, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowLeft,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    let text = self.text.buffer.as_str();
                    self.selection.move_cursor_left(text, true);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowRight,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    let text = self.text.buffer.as_str();
                    self.selection.move_cursor_right(text, true);
                }
                Event::Pointer(PointerEvent::Press {
                    button: PointerButton::Primary,
                }) => {
                    let position = Vec2::from(F64Vec2::from(input.pointer.position));
                    let byte_offset = locate_singleline_text_char(&self.text, position, ctx);
                    self.selection.cursor = byte_offset..byte_offset;
                }
                Event::Pointer(PointerEvent::Motion { .. })
                    if input.pointer.buttons.pressed(PointerButton::Primary) =>
                {
                    let position = Vec2::from(F64Vec2::from(input.pointer.position));
                    let byte_offset = locate_singleline_text_char(&self.text, position, ctx);
                    self.selection.cursor.end = byte_offset;
                }
                _ => {}
            }
        }
        self
    }

    pub fn update_if<E: Externs, F: FnOnce(&Self) -> bool>(
        self,
        f: F,
        ctx: &mut Context<E>,
        input: &input::State,
    ) -> Self {
        if f(&self) {
            self.update(ctx, input)
        } else {
            self
        }
    }

    pub fn draw<E: Externs>(self, ctx: &mut Context<E>) {
        ctx.draw_buffer.set_clip_rect(Some(self.text.rect.clone()));

        maybe_draw_singleline_text_selection(&self.text, self.selection, self.active, ctx);
        draw_singleline_text(&self.text, ctx);

        ctx.draw_buffer.set_clip_rect(None);
    }
}

// TODO: x axis scroll
pub struct TextSinglelineEditable<'a> {
    text: Text<'a>,
    selection: &'a mut TextSelection,

    hot: bool,
    active: bool,
}

impl<'a> TextSinglelineEditable<'a> {
    fn new(text: Text<'a>, selection: &'a mut TextSelection) -> Self {
        Self {
            text,
            selection,

            hot: false,
            active: false,
        }
    }

    pub fn with_hot(mut self, value: bool) -> Self {
        self.hot = value;
        self
    }

    pub fn with_active(mut self, value: bool) -> Self {
        self.active = value;
        self
    }

    pub fn is_hot(&self) -> bool {
        self.hot
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn maybe_set_hot_or_active<E: Externs>(
        mut self,
        key: Key,
        ctx: &mut Context<E>,
        input: &input::State,
    ) -> Self {
        let size = compute_singleline_text_size(&self.text, ctx);
        let interaction_rect = Rect::new(self.text.rect.min, self.text.rect.min + size);
        ctx.interaction_state
            .maybe_set_hot_or_active(key, interaction_rect, input);
        self.hot = ctx.interaction_state.is_hot(key);
        self.active = ctx.interaction_state.is_active(key);
        self
    }

    pub fn update<E: Externs>(mut self, ctx: &mut Context<E>, input: &input::State) -> Self {
        let KeyboardState { ref scancodes, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowLeft,
                    ..
                }) => {
                    let text = self.text.buffer.as_str();
                    let extend_selection =
                        scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]);
                    self.selection.move_cursor_left(text, extend_selection);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowRight,
                    ..
                }) => {
                    let text = self.text.buffer.as_str();
                    let extend_selection =
                        scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]);
                    self.selection.move_cursor_right(text, extend_selection);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::Backspace,
                    ..
                }) => {
                    let text = self.text.buffer.as_string_mut().expect("editable text");
                    self.selection.delete_left(text);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::Delete,
                    ..
                }) => {
                    let text = self.text.buffer.as_string_mut().expect("editable text");
                    self.selection.delete_right(text);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    keycode: Keycode::Char(ch),
                    ..
                }) if *ch as u32 >= 32 && *ch as u32 != 127 => {
                    // TODO: maybe better printability check ^.
                    let text = self.text.buffer.as_string_mut().expect("editable text");
                    self.selection.insert_char(text, *ch);
                }
                Event::Pointer(PointerEvent::Press {
                    button: PointerButton::Primary,
                }) => {
                    let position = Vec2::from(F64Vec2::from(input.pointer.position));
                    let byte_offset = locate_singleline_text_char(&self.text, position, ctx);
                    self.selection.cursor = byte_offset..byte_offset;
                }
                Event::Pointer(PointerEvent::Motion { .. })
                    if input.pointer.buttons.pressed(PointerButton::Primary) =>
                {
                    let position = Vec2::from(F64Vec2::from(input.pointer.position));
                    let byte_offset = locate_singleline_text_char(&self.text, position, ctx);
                    self.selection.cursor.end = byte_offset;
                }
                _ => {}
            }
        }
        self
    }

    pub fn update_if<E: Externs, F: FnOnce(&Self) -> bool>(
        self,
        f: F,
        ctx: &mut Context<E>,
        input: &input::State,
    ) -> Self {
        if f(&self) {
            self.update(ctx, input)
        } else {
            self
        }
    }

    pub fn draw<E: Externs>(self, ctx: &mut Context<E>) {
        ctx.draw_buffer.set_clip_rect(Some(self.text.rect.clone()));

        maybe_draw_singleline_text_selection(&self.text, self.selection, self.active, ctx);
        maybe_draw_singleline_text_cursor(&self.text, self.selection, self.active, ctx);
        draw_singleline_text(&self.text, ctx);

        ctx.draw_buffer.set_clip_rect(None);
    }
}

// ----

// fn compute_multiline_text_height<E: Externs>(
//     text: &str,
//     font_handle: FontHandle,
//     font_size: f32,
//     container_width: Option<f32>,
//     ctx: &mut Context<E>,
// ) -> f32 {
//     let mut font_instance_ref = ctx
//         .font_service
//         .get_font_instance_mut(font_handle, font_size);
//
//     let mut line_count: usize = 1;
//     let mut offset_x: f32 = 0.0;
//
//     for ch in text.chars() {
//         match ch {
//             '\r' => continue,
//             '\n' => {
//                 line_count += 1;
//                 offset_x = 0.0;
//             }
//             _ => {}
//         }
//
//         let char_ref = font_instance_ref.get_char(ch, &mut ctx.texture_service);
//
//         if let Some(container_width) = container_width {
//             let offset_x_end = offset_x + char_ref.advance_width();
//             if offset_x_end > container_width {
//                 line_count += 1;
//                 offset_x = 0.0;
//             }
//         }
//
//         offset_x += char_ref.advance_width();
//     }
//
//     line_count as f32 * font_instance_ref.height()
// }
