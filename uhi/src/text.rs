use std::ops::Range;

use input::{Event, KeyboardEvent, KeyboardState, Keycode, PointerButton, PointerEvent, Scancode};

use crate::{
    Context, DrawBuffer, Externs, F64Vec2, Fill, FillTexture, FontHandle, FontInstanceRefMut, Key,
    Rect, RectShape, Rgba8, TextureKind, TextureService, Vec2,
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
    // used only in editors.
    // TODO: consider animating scroll.
    scroll_x: f32,
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

    pub fn multiline(self) -> TextMultiline<'a> {
        TextMultiline::new(self)
    }
}

// ----
// update-related functions for singleline text

fn compute_singleline_text_size<E: Externs>(text: &Text, ctx: &mut Context<E>) -> Vec2 {
    let mut font_instance_ref = ctx
        .font_service
        .get_font_instance_mut(text.font_handle, text.font_size);
    let text_width =
        font_instance_ref.compute_text_width(text.buffer.as_str(), &mut ctx.texture_service);
    Vec2::new(text_width, font_instance_ref.height())
}

// returns byte offset(not char index)
fn locate_singleline_text_char<E: Externs>(
    text: &Text,
    position: Vec2,
    scroll_x: f32,
    ctx: &mut Context<E>,
) -> usize {
    let min_x = text.rect.min.x - scroll_x;

    // maybe we're dragging and the pointer is before beginning of the line.
    if position.x < min_x {
        return 0;
    }

    let mut font_instance_ref = ctx
        .font_service
        .get_font_instance_mut(text.font_handle, text.font_size);

    let mut byte_offset: usize = 0;
    let mut offset_x: f32 = min_x;

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

// ----
// draw-related functions

fn draw_singleline_text_selection<E: Externs>(
    text: &Text,
    selection: &TextSelection,
    selection_start_x: f32,
    selection_end_x: f32,
    active: bool,
    font_instance_ref: &mut FontInstanceRefMut,
    draw_buffer: &mut DrawBuffer<E>,
) {
    // NOTE: end is where the cursor is. for example in `hello, sailor` selection may have started
    // at `,` and moved left to `e`.
    let left = selection_start_x.min(selection_end_x);
    let right = selection_start_x.max(selection_end_x);

    let min = text.rect.min - Vec2::new(selection.scroll_x, 0.0) + Vec2::new(left, 0.0);
    let size = Vec2::new(right - left, font_instance_ref.height());
    let rect = Rect::new(min, min + size);
    let fill = if active {
        text.palette
            .as_ref()
            .map_or_else(|| SELECTION_ACTIVE, |a| a.selection_active)
    } else {
        text.palette
            .as_ref()
            .map_or_else(|| SELECTION_INACTIVE, |a| a.selection_inactive)
    };
    draw_buffer.push_rect(RectShape::with_fill(rect, Fill::with_color(fill)));
}

fn draw_singleline_text<E: Externs>(
    text: &Text,
    scroll_x: f32,
    font_instance_ref: &mut FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
    draw_buffer: &mut DrawBuffer<E>,
) {
    let ascent = font_instance_ref.ascent();
    let fg = text.palette.as_ref().map(|a| a.fg).unwrap_or(FG);

    let mut offset_x: f32 = text.rect.min.x - scroll_x;
    for ch in text.buffer.as_str().chars() {
        let char_ref = font_instance_ref.get_char(ch, texture_service);

        draw_buffer.push_rect(RectShape::with_fill(
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

// ----
// singleline text

pub struct TextSingleline<'a> {
    text: Text<'a>,
}

impl<'a> TextSingleline<'a> {
    fn new(text: Text<'a>) -> Self {
        Self { text }
    }

    pub fn draw<E: Externs>(self, ctx: &mut Context<E>) {
        ctx.draw_buffer.set_clip_rect(Some(self.text.rect.clone()));

        let mut font_instance_ref = ctx
            .font_service
            .get_font_instance_mut(self.text.font_handle, self.text.font_size);
        draw_singleline_text(
            &self.text,
            0.0,
            &mut font_instance_ref,
            &mut ctx.texture_service,
            &mut ctx.draw_buffer,
        );

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
                    let byte_offset = locate_singleline_text_char(
                        &self.text,
                        position,
                        self.selection.scroll_x,
                        ctx,
                    );
                    self.selection.cursor = byte_offset..byte_offset;
                }
                Event::Pointer(PointerEvent::Motion { .. })
                    if input.pointer.buttons.pressed(PointerButton::Primary) =>
                {
                    let position = Vec2::from(F64Vec2::from(input.pointer.position));
                    let byte_offset = locate_singleline_text_char(
                        &self.text,
                        position,
                        self.selection.scroll_x,
                        ctx,
                    );
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

        let mut font_instance_ref = ctx
            .font_service
            .get_font_instance_mut(self.text.font_handle, self.text.font_size);

        if !self.selection.is_empty() {
            let selection_start_x = font_instance_ref.compute_text_width(
                &self.text.buffer.as_str()[..self.selection.cursor.start],
                &mut ctx.texture_service,
            );
            let selection_end_x = font_instance_ref.compute_text_width(
                &self.text.buffer.as_str()[..self.selection.cursor.end],
                &mut ctx.texture_service,
            );
            draw_singleline_text_selection(
                &self.text,
                self.selection,
                selection_start_x,
                selection_end_x,
                self.active,
                &mut font_instance_ref,
                &mut ctx.draw_buffer,
            );
        }

        draw_singleline_text(
            &self.text,
            0.0,
            &mut font_instance_ref,
            &mut ctx.texture_service,
            &mut ctx.draw_buffer,
        );

        ctx.draw_buffer.set_clip_rect(None);
    }
}

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
                    let byte_offset = locate_singleline_text_char(
                        &self.text,
                        position,
                        self.selection.scroll_x,
                        ctx,
                    );
                    self.selection.cursor = byte_offset..byte_offset;
                }
                Event::Pointer(PointerEvent::Motion { .. })
                    if input.pointer.buttons.pressed(PointerButton::Primary) =>
                {
                    let position = Vec2::from(F64Vec2::from(input.pointer.position));
                    let byte_offset = locate_singleline_text_char(
                        &self.text,
                        position,
                        self.selection.scroll_x,
                        ctx,
                    );
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

        let mut font_instance_ref = ctx
            .font_service
            .get_font_instance_mut(self.text.font_handle, self.text.font_size);

        let rect_width = self.text.rect.width();
        let text_width = font_instance_ref
            .compute_text_width(&self.text.buffer.as_str(), &mut ctx.texture_service);
        let cursor_width = font_instance_ref.typical_advance_width();

        let selection_start_x = font_instance_ref.compute_text_width(
            &self.text.buffer.as_str()[..self.selection.cursor.start],
            &mut ctx.texture_service,
        );
        let selection_end_x = font_instance_ref.compute_text_width(
            &self.text.buffer.as_str()[..self.selection.cursor.end],
            &mut ctx.texture_service,
        );

        let mut scroll_x = self.selection.scroll_x;
        // right edge. scroll to show cursor + overscroll for cursor width.
        if selection_end_x + cursor_width - scroll_x > rect_width {
            scroll_x = selection_end_x + cursor_width - rect_width;
        }
        // left edge. scroll to show cursor.
        if selection_end_x < scroll_x {
            scroll_x = selection_end_x;
        }
        // undo overscroll when cursor moves back. if we can show all text without overscrolling,
        // do that.
        if selection_end_x + cursor_width <= text_width && text_width > rect_width {
            scroll_x = scroll_x.min(text_width - rect_width);
        }
        self.selection.scroll_x = scroll_x;

        if !self.selection.is_empty() {
            draw_singleline_text_selection(
                &self.text,
                self.selection,
                selection_start_x,
                selection_end_x,
                self.active,
                &mut font_instance_ref,
                &mut ctx.draw_buffer,
            );
        }

        if self.active {
            let min = self.text.rect.min - Vec2::new(self.selection.scroll_x, 0.0)
                + Vec2::new(selection_end_x, 0.0);
            let size = Vec2::new(cursor_width, font_instance_ref.height());
            let rect = Rect::new(min, min + size);
            let fill = self
                .text
                .palette
                .as_ref()
                .map_or_else(|| CURSOR, |a| a.cursor);
            ctx.draw_buffer
                .push_rect(RectShape::with_fill(rect, Fill::with_color(fill)));
        }

        draw_singleline_text(
            &self.text,
            scroll_x,
            &mut font_instance_ref,
            &mut ctx.texture_service,
            &mut ctx.draw_buffer,
        );

        ctx.draw_buffer.set_clip_rect(None);
    }
}

// ----
// reusable update-related functions for multline text

// TODO: y scroll or something. i want to be able to "scroll to bottom".
fn draw_multiline_text<E: Externs>(text: &Text, ctx: &mut Context<E>) {
    let mut font_instance_ref = ctx
        .font_service
        .get_font_instance_mut(text.font_handle, text.font_size);
    let ascent = font_instance_ref.ascent();

    let fg = text.palette.as_ref().map(|a| a.fg).unwrap_or(FG);

    let mut offset_x: f32 = text.rect.min.x;
    let mut offset_y: f32 = text.rect.min.y;
    for ch in text.buffer.as_str().chars() {
        if ch == '\r' {
            continue;
        }
        if ch == '\n' {
            offset_x = text.rect.min.x;
            offset_y += font_instance_ref.height();
            continue;
        }

        let char_ref = font_instance_ref.get_char(ch, &mut ctx.texture_service);

        ctx.draw_buffer.push_rect(RectShape::with_fill(
            char_ref
                .bounding_rect()
                .translate_by(&Vec2::new(offset_x, offset_y + ascent)),
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

// ----
// multiline text

pub struct TextMultiline<'a> {
    text: Text<'a>,
}

impl<'a> TextMultiline<'a> {
    fn new(text: Text<'a>) -> Self {
        Self { text }
    }

    pub fn draw<E: Externs>(self, ctx: &mut Context<E>) {
        ctx.draw_buffer.set_clip_rect(Some(self.text.rect.clone()));

        draw_multiline_text(&self.text, ctx);

        ctx.draw_buffer.set_clip_rect(None);
    }

    // pub fn selectable(self, selection: &'a mut TextSelection) -> TextSinglelineSelectable<'a> {
    //     todo!()
    // }
    //
    // pub fn editable(self, selection: &'a mut TextSelection) -> TextSinglelineEditable<'a> {
    //     todo!()
    // }
}

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
