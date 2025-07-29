use std::ops::Range;

use input::{
    CursorShape, Event, KeyboardEvent, KeyboardState, Keycode, PointerButton, PointerEvent,
    Scancode,
};

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
// i don't quite like the idea of palette (or style) hierarchies. ensure that styles struct is
// flat.

// TODO: text's maybe_set_hot_or_active must accept an interaction rect enum that would instruct
// the function to compute minimal rect that would be able to accomodate the text, use rect that
// was provided during construction or would allow user to specify custom interaction rect.

const FG: Rgba8 = Rgba8::WHITE;
const SELECTION_ACTIVE: Rgba8 = Rgba8::from_u32(0x304a3dff);
const SELECTION_INACTIVE: Rgba8 = Rgba8::from_u32(0x484848ff);
const CURSOR: Rgba8 = Rgba8::from_u32(0x8faf9fff);

#[derive(Clone)]
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

    fn normalized_cursor(&self) -> Range<usize> {
        let left = self.cursor.start.min(self.cursor.end);
        let right = self.cursor.start.max(self.cursor.end);
        left..right
    }

    fn delete_selection(&mut self, text: &mut String) {
        let normalized_cursor = self.normalized_cursor();
        if normalized_cursor.end > normalized_cursor.start {
            text.replace_range(normalized_cursor, "");
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

    fn paste(&mut self, text: &mut String, pasta: &str) {
        let normalized_cursor = self.normalized_cursor();
        if self.is_empty() {
            text.insert_str(normalized_cursor.start, pasta);
        } else {
            text.replace_range(normalized_cursor.clone(), pasta);
        }
        self.cursor.end = normalized_cursor.start + pasta.len();
        self.cursor.start = self.cursor.end;
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
    rect: Rect,
    font_handle: Option<FontHandle>,
    font_size: Option<f32>,
    palette: Option<TextPalette>,
}

impl<'a> Text<'a> {
    pub fn new<B: Into<TextBuffer<'a>>>(text: B, rect: Rect) -> Self {
        Self {
            buffer: text.into(),
            rect,
            font_handle: None,
            font_size: None,
            palette: None,
        }
    }

    pub fn with_font_handle(mut self, value: FontHandle) -> Self {
        self.font_handle = Some(value);
        self
    }

    pub fn with_font_size(mut self, value: f32) -> Self {
        self.font_size = Some(value);
        self
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
// update singleline stuff

fn compute_singleline_text_size<E: Externs>(text: &Text, ctx: &mut Context<E>) -> Vec2 {
    let mut font_instance = ctx.font_service.get_font_instance(
        text.font_handle.unwrap_or(ctx.default_font_handle()),
        text.font_size.unwrap_or(ctx.default_font_size()),
    );
    let text_width =
        font_instance.compute_text_width(text.buffer.as_str(), &mut ctx.texture_service);
    Vec2::new(text_width, font_instance.height())
}

// returns byte offset(not char index)
fn locate_singleline_text_coord<E: Externs>(
    s: &str,
    min_x: f32,
    position: Vec2,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> usize {
    // maybe we're dragging and the pointer is before beginning of the line.
    if position.x < min_x {
        return 0;
    }

    let mut byte_offset: usize = 0;
    let mut offset_x: f32 = min_x;
    for ch in s.chars() {
        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);

        let start_x = offset_x;
        let end_x = start_x + glyph.advance_width();
        if position.x >= start_x && position.x <= end_x {
            // NOTE: it seems like everyone consider char selected only if you're reaching past
            // half of it.
            let mid_x = start_x + glyph.advance_width() / 2.0;
            if position.x < mid_x {
                return byte_offset;
            } else {
                return byte_offset + ch.len_utf8();
            }
        }

        byte_offset += ch.len_utf8();
        offset_x += glyph.advance_width();
    }

    // the pointer is after end of the line.
    assert!(position.x > offset_x);
    s.len()
}

// ----
// draw singleline stuff

fn draw_singleline_text_selection<E: Externs>(
    text: &Text,
    selection: &TextSelection,
    selection_start_x: f32,
    selection_end_x: f32,
    active: bool,
    font_instance: &mut FontInstanceRefMut,
    draw_buffer: &mut DrawBuffer<E>,
) {
    // NOTE: end is where the cursor is. for example in `hello, sailor` selection may have started
    // at `,` and moved left to `e`.
    let left = selection_start_x.min(selection_end_x);
    let right = selection_start_x.max(selection_end_x);

    let min = text.rect.min - Vec2::new(selection.scroll_x, 0.0) + Vec2::new(left, 0.0);
    let size = Vec2::new(right - left, font_instance.height());
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
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
    draw_buffer: &mut DrawBuffer<E>,
) {
    let ascent = font_instance.ascent();
    let fg = text.palette.as_ref().map(|a| a.fg).unwrap_or(FG);

    let mut offset_x: f32 = text.rect.min.x - scroll_x;
    for ch in text.buffer.as_str().chars() {
        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);

        draw_buffer.push_rect(RectShape::with_fill(
            glyph
                .bounding_rect()
                .translate_by(&Vec2::new(offset_x, text.rect.min.y + ascent)),
            Fill::new(
                fg,
                FillTexture {
                    kind: TextureKind::Internal(glyph.tex_handle()),
                    coords: glyph.tex_coords(),
                },
            ),
        ));

        offset_x += glyph.advance_width();
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
        ctx.draw_buffer.set_clip_rect(Some(self.text.rect));

        let font_instance = ctx.font_service.get_font_instance(
            self.text.font_handle.unwrap_or(ctx.default_font_handle()),
            self.text.font_size.unwrap_or(ctx.default_font_size()),
        );
        draw_singleline_text(
            &self.text,
            0.0,
            font_instance,
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
        ctx.maybe_set_hot_or_active(key, interaction_rect, CursorShape::Text, input);
        self.hot = ctx.is_hot(key);
        self.active = ctx.is_active(key);
        self
    }

    pub fn update<E: Externs>(self, key: Key, ctx: &mut Context<E>, input: &input::State) -> Self {
        let KeyboardState { ref scancodes, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowLeft,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    let s = self.text.buffer.as_str();
                    self.selection.move_cursor_left(s, true);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowRight,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    let s = self.text.buffer.as_str();
                    self.selection.move_cursor_right(s, true);
                }
                Event::Pointer(
                    pe @ PointerEvent::Press {
                        button: PointerButton::Primary,
                    },
                )
                | Event::Pointer(pe @ PointerEvent::Motion { .. })
                    if input.pointer.buttons.pressed(PointerButton::Primary) =>
                {
                    let font_instance = ctx.font_service.get_font_instance(
                        self.text.font_handle.unwrap_or(ctx.default_font_handle()),
                        self.text.font_size.unwrap_or(ctx.default_font_size()),
                    );
                    let byte_offset = locate_singleline_text_coord(
                        self.text.buffer.as_str(),
                        self.text.rect.min.x - self.selection.scroll_x,
                        Vec2::from(F64Vec2::from(input.pointer.position)),
                        font_instance,
                        &mut ctx.texture_service,
                    );
                    if let PointerEvent::Press { .. } = pe {
                        self.selection.cursor = byte_offset..byte_offset;
                    } else {
                        self.selection.cursor.end = byte_offset;
                    }
                }
                _ => {}
            }
        }
        self
    }

    pub fn update_if<E: Externs, F: FnOnce(&Self) -> bool>(
        self,
        f: F,
        key: Key,
        ctx: &mut Context<E>,
        input: &input::State,
    ) -> Self {
        if f(&self) {
            self.update(key, ctx, input)
        } else {
            self
        }
    }

    pub fn draw<E: Externs>(self, ctx: &mut Context<E>) {
        ctx.draw_buffer.set_clip_rect(Some(self.text.rect));

        let mut font_instance = ctx.font_service.get_font_instance(
            self.text.font_handle.unwrap_or(ctx.default_font_handle()),
            self.text.font_size.unwrap_or(ctx.default_font_size()),
        );

        if !self.selection.is_empty() {
            let s = self.text.buffer.as_str();
            let selection_start_x = font_instance
                .compute_text_width(&s[..self.selection.cursor.start], &mut ctx.texture_service);
            let selection_end_x = font_instance
                .compute_text_width(&s[..self.selection.cursor.end], &mut ctx.texture_service);
            draw_singleline_text_selection(
                &self.text,
                self.selection,
                selection_start_x,
                selection_end_x,
                self.active,
                &mut font_instance,
                &mut ctx.draw_buffer,
            );
        }

        draw_singleline_text(
            &self.text,
            0.0,
            font_instance,
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
        ctx.maybe_set_hot_or_active(key, interaction_rect, CursorShape::Text, input);
        self.hot = ctx.is_hot(key);
        self.active = ctx.is_active(key);
        self
    }

    pub fn update<E: Externs>(
        mut self,
        key: Key,
        ctx: &mut Context<E>,
        input: &input::State,
    ) -> Self {
        let KeyboardState { ref scancodes, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowLeft,
                    ..
                }) => {
                    let s = self.text.buffer.as_str();
                    let extend_selection =
                        scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]);
                    self.selection.move_cursor_left(s, extend_selection);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowRight,
                    ..
                }) => {
                    let s = self.text.buffer.as_str();
                    let extend_selection =
                        scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]);
                    self.selection.move_cursor_right(s, extend_selection);
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
                    scancode: Scancode::V,
                    ..
                }) if scancodes.any_pressed([Scancode::CtrlLeft, Scancode::CtrlRight]) => {
                    ctx.request_clipboard_read(key);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    keycode: Keycode::Char(ch),
                    ..
                }) if *ch as u32 >= 32 && *ch as u32 != 127 => {
                    // TODO: maybe better printability check ^.
                    let text = self.text.buffer.as_string_mut().expect("editable text");
                    self.selection.insert_char(text, *ch);
                }
                Event::Pointer(
                    ev @ PointerEvent::Press {
                        button: PointerButton::Primary,
                    },
                )
                | Event::Pointer(ev @ PointerEvent::Motion { .. })
                    if input.pointer.buttons.pressed(PointerButton::Primary) =>
                {
                    let font_instance = ctx.font_service.get_font_instance(
                        self.text.font_handle.unwrap_or(ctx.default_font_handle()),
                        self.text.font_size.unwrap_or(ctx.default_font_size()),
                    );
                    let byte_offset = locate_singleline_text_coord(
                        self.text.buffer.as_str(),
                        self.text.rect.min.x - self.selection.scroll_x,
                        Vec2::from(F64Vec2::from(input.pointer.position)),
                        font_instance,
                        &mut ctx.texture_service,
                    );
                    if let PointerEvent::Press { .. } = ev {
                        self.selection.cursor = byte_offset..byte_offset;
                    } else {
                        self.selection.cursor.end = byte_offset;
                    }
                }
                _ => {}
            }
        }

        if let Some(pasta) = ctx.take_clipboard_read(key) {
            let text = self.text.buffer.as_string_mut().expect("editable text");
            // TODO: consider removing line breaks or something.
            self.selection.paste(text, pasta.as_str());
        }

        self
    }

    pub fn update_if<E: Externs, F: FnOnce(&Self) -> bool>(
        self,
        f: F,
        key: Key,
        ctx: &mut Context<E>,
        input: &input::State,
    ) -> Self {
        if f(&self) {
            self.update(key, ctx, input)
        } else {
            self
        }
    }

    pub fn draw<E: Externs>(self, ctx: &mut Context<E>) {
        ctx.draw_buffer.set_clip_rect(Some(self.text.rect));

        let s = self.text.buffer.as_str();
        let mut font_instance = ctx.font_service.get_font_instance(
            self.text.font_handle.unwrap_or(ctx.default_font_handle()),
            self.text.font_size.unwrap_or(ctx.default_font_size()),
        );

        let rect_width = self.text.rect.width();
        let text_width = font_instance.compute_text_width(s, &mut ctx.texture_service);
        let cursor_width = font_instance.typical_advance_width();

        let selection_start_x = font_instance
            .compute_text_width(&s[..self.selection.cursor.start], &mut ctx.texture_service);
        let selection_end_x = font_instance
            .compute_text_width(&s[..self.selection.cursor.end], &mut ctx.texture_service);

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
                &mut font_instance,
                &mut ctx.draw_buffer,
            );
        }

        if self.active {
            let min = self.text.rect.min - Vec2::new(self.selection.scroll_x, 0.0)
                + Vec2::new(selection_end_x, 0.0);
            let size = Vec2::new(cursor_width, font_instance.height());
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
            font_instance,
            &mut ctx.texture_service,
            &mut ctx.draw_buffer,
        );

        ctx.draw_buffer.set_clip_rect(None);
    }
}

// ----
// update multline stuff

fn compute_multiline_text_height<E: Externs>(
    text: &Text,
    // container_width: Option<f32>,
    ctx: &mut Context<E>,
) -> f32 {
    let font_instance = ctx.font_service.get_font_instance(
        text.font_handle.unwrap_or(ctx.default_font_handle()),
        text.font_size.unwrap_or(ctx.default_font_size()),
    );

    let mut line_count: usize = 1;
    // let mut offset_x: f32 = 0.0;
    for ch in text.buffer.as_str().chars() {
        if ch == '\r' {
            continue;
        }
        if ch == '\n' {
            line_count += 1;
            // offset_x = 0.0;
            continue;
        }

        // TODO: multiline text wrapping
        //
        // let glyph = font_instance.get_char(ch, &mut ctx.texture_service);
        // if let Some(container_width) = container_width {
        //     let offset_x_end = offset_x + glyph.advance_width();
        //     if offset_x_end > container_width {
        //         line_count += 1;
        //         offset_x = 0.0;
        //     }
        // }
        // offset_x += glyph.advance_width();
    }

    line_count as f32 * font_instance.height()
}

fn locate_multiline_text_coord<E: Externs>(
    text: &Text,
    position: Vec2,
    ctx: &mut Context<E>,
) -> usize {
    let s = text.buffer.as_str();
    let font_instance = ctx.font_service.get_font_instance(
        text.font_handle.unwrap_or(ctx.default_font_handle()),
        text.font_size.unwrap_or(ctx.default_font_size()),
    );

    let mut line_start_idx: usize = 0;
    let mut offset_y: f32 = text.rect.min.y;
    while let Some(i) = s[line_start_idx..].find('\n') {
        let start_y = offset_y;
        if line_start_idx == 0 && position.y < start_y {
            return 0;
        }

        let end_y = start_y + font_instance.height();
        if position.y >= start_y && position.y <= end_y {
            break;
        }

        // NOTE: +1 to skip '\n'
        line_start_idx += i + 1;
        offset_y += font_instance.height();
    }

    // maybe the pointer is below?
    if line_start_idx >= s.len() {
        return s.len();
    }

    // maybe we're dragging and the pointer is before beginning of the line.
    if position.x < text.rect.min.x {
        return line_start_idx;
    }

    // ----

    let line_end_idx = s[line_start_idx..]
        .find('\n')
        .map_or_else(|| s.len(), |i| line_start_idx + i);
    let line = &s[line_start_idx..line_end_idx];
    line_start_idx
        + locate_singleline_text_coord(
            line,
            text.rect.min.x,
            position,
            font_instance,
            &mut ctx.texture_service,
        )
}

// ----
// draw multline stuff

// TODO: y scroll or something. i want to be able to "scroll to bottom".
fn draw_multiline_text<E: Externs>(
    text: &Text,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
    draw_buffer: &mut DrawBuffer<E>,
) {
    let ascent = font_instance.ascent();
    let fg = text.palette.as_ref().map(|a| a.fg).unwrap_or(FG);

    let mut offset_x: f32 = text.rect.min.x;
    let mut offset_y: f32 = text.rect.min.y;
    for ch in text.buffer.as_str().chars() {
        if ch == '\r' {
            continue;
        }
        if ch == '\n' {
            offset_x = text.rect.min.x;
            offset_y += font_instance.height();
            continue;
        }

        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);

        draw_buffer.push_rect(RectShape::with_fill(
            glyph
                .bounding_rect()
                .translate_by(&Vec2::new(offset_x, offset_y + ascent)),
            Fill::new(
                fg,
                FillTexture {
                    kind: TextureKind::Internal(glyph.tex_handle()),
                    coords: glyph.tex_coords(),
                },
            ),
        ));

        offset_x += glyph.advance_width();
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
        ctx.draw_buffer.set_clip_rect(Some(self.text.rect));

        let font_instance = ctx.font_service.get_font_instance(
            self.text.font_handle.unwrap_or(ctx.default_font_handle()),
            self.text.font_size.unwrap_or(ctx.default_font_size()),
        );
        draw_multiline_text(
            &self.text,
            font_instance,
            &mut ctx.texture_service,
            &mut ctx.draw_buffer,
        );

        ctx.draw_buffer.set_clip_rect(None);
    }

    pub fn selectable(self, selection: &'a mut TextSelection) -> TextMultilineSelectable<'a> {
        TextMultilineSelectable::new(self.text, selection)
    }

    // pub fn editable(self, selection: &'a mut TextSelection) -> TextMultilineEditable<'a> {
    //     todo!()
    // }
}

pub struct TextMultilineSelectable<'a> {
    text: Text<'a>,
    selection: &'a mut TextSelection,

    hot: bool,
    active: bool,
}

impl<'a> TextMultilineSelectable<'a> {
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
        // TODO: do i need to compute multiline text height here really? wouldn't it make make
        // sense for the "text area" to reserve the entirety of available space?
        let height = compute_multiline_text_height(&self.text, ctx);
        // NOTE: multline text currently takes the whole width of the provided rect. that is
        // probably fine, isn't it?
        let size = Vec2::new(self.text.rect.max.x, height);
        let interaction_rect = Rect::new(self.text.rect.min, self.text.rect.min + size);
        ctx.maybe_set_hot_or_active(key, interaction_rect, CursorShape::Text, input);
        self.hot = ctx.is_hot(key);
        self.active = ctx.is_active(key);
        self
    }

    pub fn update<E: Externs>(self, key: Key, ctx: &mut Context<E>, input: &input::State) -> Self {
        let KeyboardState { ref scancodes, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowLeft,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    let s = self.text.buffer.as_str();
                    self.selection.move_cursor_left(s, true);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowRight,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    let s = self.text.buffer.as_str();
                    self.selection.move_cursor_right(s, true);
                }
                Event::Pointer(
                    pe @ PointerEvent::Press {
                        button: PointerButton::Primary,
                    },
                )
                | Event::Pointer(pe @ PointerEvent::Motion { .. })
                    if input.pointer.buttons.pressed(PointerButton::Primary) =>
                {
                    let byte_offset = locate_multiline_text_coord(
                        &self.text,
                        Vec2::from(F64Vec2::from(input.pointer.position)),
                        ctx,
                    );
                    if let PointerEvent::Press { .. } = pe {
                        self.selection.cursor = byte_offset..byte_offset;
                    } else {
                        self.selection.cursor.end = byte_offset;
                    }
                }
                _ => {}
            }
        }
        self
    }

    pub fn update_if<E: Externs, F: FnOnce(&Self) -> bool>(
        self,
        f: F,
        key: Key,
        ctx: &mut Context<E>,
        input: &input::State,
    ) -> Self {
        if f(&self) {
            self.update(key, ctx, input)
        } else {
            self
        }
    }

    pub fn draw<E: Externs>(self, ctx: &mut Context<E>) {
        ctx.draw_buffer.set_clip_rect(Some(self.text.rect));

        let mut font_instance = ctx.font_service.get_font_instance(
            self.text.font_handle.unwrap_or(ctx.default_font_handle()),
            self.text.font_size.unwrap_or(ctx.default_font_size()),
        );

        // TODO: extract draw_multiline_text_selection
        if !self.selection.is_empty() {
            let s = self.text.buffer.as_str();
            let fill = if self.active {
                self.text
                    .palette
                    .as_ref()
                    .map_or_else(|| SELECTION_ACTIVE, |a| a.selection_active)
            } else {
                self.text
                    .palette
                    .as_ref()
                    .map_or_else(|| SELECTION_INACTIVE, |a| a.selection_inactive)
            };

            let start = self.selection.cursor.start.min(self.selection.cursor.end);
            let end = self.selection.cursor.start.max(self.selection.cursor.end);

            let mut line_start_idx: usize = 0;
            let mut offset_y: f32 = 0.0;
            loop {
                let line_end_idx = s[line_start_idx..]
                    .find('\n')
                    .map_or_else(|| s.len(), |i| line_start_idx + i);

                if line_start_idx < end && line_end_idx > start {
                    let start_in_line = start.max(line_start_idx);
                    let end_in_line = end.min(line_end_idx);

                    let start_x = font_instance.compute_text_width(
                        &s[line_start_idx..start_in_line],
                        &mut ctx.texture_service,
                    );
                    let end_x = font_instance.compute_text_width(
                        &s[line_start_idx..end_in_line],
                        &mut ctx.texture_service,
                    );

                    let min = self.text.rect.min + Vec2::new(start_x, offset_y);
                    let size = Vec2::new(end_x - start_x, font_instance.height());
                    let rect = Rect::new(min, min + size);
                    ctx.draw_buffer
                        .push_rect(RectShape::with_fill(rect, Fill::with_color(fill)));
                }

                if line_end_idx + 1 >= end.min(s.len()) {
                    break;
                }

                // NOTE: +1 to skip '\n'
                line_start_idx = line_end_idx + 1;
                offset_y += font_instance.height();
            }
        }

        draw_multiline_text(
            &self.text,
            font_instance,
            &mut ctx.texture_service,
            &mut ctx.draw_buffer,
        );

        ctx.draw_buffer.set_clip_rect(None);
    }
}
