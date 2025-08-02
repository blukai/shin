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

// TODO: make keyboard keys configurable. that would allow to have platform-specific definitions as
// well as user-provided.
// see "Text-editing shortcuts" at https://support.apple.com/en-us/102650.

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

    fn normalized_cursor(&self) -> Range<usize> {
        let left = self.cursor.start.min(self.cursor.end);
        let right = self.cursor.start.max(self.cursor.end);
        left..right
    }

    // TODO: move modifiers (by char, by char type, by word, etc.)

    fn move_left(&mut self, text: &str, extend_selection: bool) {
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

    fn move_right(&mut self, text: &str, extend_selection: bool) {
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

    fn move_home(&mut self, text: &str, extend_selection: bool) {
        self.cursor.end = text[..self.cursor.end]
            .rfind('\n')
            .map_or_else(|| 0, |i| i + 1);
        if !extend_selection {
            self.cursor.start = self.cursor.end;
        }
    }

    fn move_end(&mut self, text: &str, extend_selection: bool) {
        self.cursor.end = text[self.cursor.end..]
            .find('\n')
            .map_or_else(|| text.len(), |i| self.cursor.end + i);
        if !extend_selection {
            self.cursor.start = self.cursor.end;
        }
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
            self.move_left(text, true);
        }
        self.delete_selection(text);
    }

    fn delete_right(&mut self, text: &mut String) {
        if self.is_empty() {
            self.cursor.end = self.cursor.start;
            self.move_right(text, true);
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

    fn copy<'a>(&self, text: &'a str) -> Option<&'a str> {
        if self.is_empty() {
            return None;
        }
        let normalized_cursor = self.normalized_cursor();
        Some(&text[normalized_cursor])
    }
}

// NOTE: this is marked non_exhaustive because i don't want this to be constructable from outside.
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

        let min_x = offset_x;
        let max_x = min_x + glyph.advance_width();
        if position.x >= min_x && position.x <= max_x {
            // NOTE: it seems like everyone consider char selected only if you're reaching past
            // half of it.
            let center_x = min_x + (max_x - min_x) / 2.0;
            if position.x < center_x {
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

// TODO: draw_singleline_text_selection should also draw cursor (if draw_cursor is on or
// something).
//
// TODO: rename draw_singleline_text_selection to draw_singleline_selection.
fn draw_singleline_text_selection<E: Externs>(
    text: &Text,
    selection_min_x: f32,
    selection_max_x: f32,
    scroll_x: f32,
    active: bool,
    font_instance: &mut FontInstanceRefMut,
    draw_buffer: &mut DrawBuffer<E>,
) {
    // NOTE: end is where the cursor is. for example in `hello, sailor` selection may have started
    // at `,` and moved left to `e`.
    let min_x = selection_min_x.min(selection_max_x);
    let max_x = selection_min_x.max(selection_max_x);

    let min = text.rect.min - Vec2::new(scroll_x, 0.0) + Vec2::new(min_x, 0.0);
    let size = Vec2::new(max_x - min_x, font_instance.height());
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
    let font_ascent = font_instance.ascent();
    let fg = text.palette.as_ref().map(|a| a.fg).unwrap_or(FG);

    let mut offset_x: f32 = text.rect.min.x - scroll_x;
    for ch in text.buffer.as_str().chars() {
        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);

        draw_buffer.push_rect(RectShape::with_fill(
            glyph
                .bounding_rect()
                .translate_by(&Vec2::new(offset_x, text.rect.min.y + font_ascent)),
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

    pub fn update<E: Externs>(self, _key: Key, ctx: &mut Context<E>, input: &input::State) -> Self {
        let KeyboardState { ref scancodes, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowLeft,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    self.selection.move_left(self.text.buffer.as_str(), true);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowRight,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    self.selection.move_right(self.text.buffer.as_str(), true);
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::Home,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    self.selection.move_home(self.text.buffer.as_str(), true);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::End,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    self.selection.move_end(self.text.buffer.as_str(), true);
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::C,
                    ..
                }) if scancodes.any_pressed([Scancode::CtrlLeft, Scancode::CtrlRight]) => {
                    let text = self.text.buffer.as_str();
                    if let Some(copy) = self.selection.copy(text) {
                        // TODO: consider allocating copy into single-frame arena or something.
                        ctx.request_clipboard_write(copy.to_string());
                    }
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
            let text = self.text.buffer.as_str();
            let selection_min_x = font_instance.compute_text_width(
                &text[..self.selection.cursor.start],
                &mut ctx.texture_service,
            );
            // TODO: don't recompute prefix width, sum prefix and "infix".
            let selection_max_x = font_instance
                .compute_text_width(&text[..self.selection.cursor.end], &mut ctx.texture_service);
            draw_singleline_text_selection(
                &self.text,
                selection_min_x,
                selection_max_x,
                self.selection.scroll_x,
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
                    self.selection.move_left(
                        self.text.buffer.as_str(),
                        scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]),
                    );
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowRight,
                    ..
                }) => {
                    self.selection.move_right(
                        self.text.buffer.as_str(),
                        scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]),
                    );
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::Home,
                    ..
                }) => {
                    self.selection.move_home(
                        self.text.buffer.as_str(),
                        scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]),
                    );
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::End,
                    ..
                }) => {
                    self.selection.move_end(
                        self.text.buffer.as_str(),
                        scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]),
                    );
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::V,
                    ..
                }) if scancodes.any_pressed([Scancode::CtrlLeft, Scancode::CtrlRight]) => {
                    ctx.request_clipboard_read(key);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::C,
                    ..
                }) if scancodes.any_pressed([Scancode::CtrlLeft, Scancode::CtrlRight]) => {
                    let text = self.text.buffer.as_string_mut().unwrap();
                    if let Some(copy) = self.selection.copy(text) {
                        // TODO: consider allocating copy into single-frame arena or something.
                        ctx.request_clipboard_write(copy.to_string());
                    }
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::X,
                    ..
                }) if scancodes.any_pressed([Scancode::CtrlLeft, Scancode::CtrlRight]) => {
                    let text = self.text.buffer.as_string_mut().unwrap();
                    if let Some(copy) = self.selection.copy(text) {
                        // TODO: consider allocating copy into single-frame arena or something.
                        ctx.request_clipboard_write(copy.to_string());
                        self.selection.delete_selection(text);
                    }
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::Backspace,
                    ..
                }) => {
                    self.selection
                        .delete_left(self.text.buffer.as_string_mut().unwrap());
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::Delete,
                    ..
                }) => {
                    self.selection
                        .delete_right(self.text.buffer.as_string_mut().unwrap());
                }
                Event::Keyboard(KeyboardEvent::Press {
                    keycode: Keycode::Char(ch),
                    ..
                }) if *ch as u32 >= 32 && *ch as u32 != 127 => {
                    // TODO: maybe better printability check ^.
                    self.selection
                        .insert_char(self.text.buffer.as_string_mut().unwrap(), *ch);
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
            // TODO: consider removing line breaks or something.
            self.selection
                .paste(self.text.buffer.as_string_mut().unwrap(), pasta.as_str());
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

        let text = self.text.buffer.as_str();
        let mut font_instance = ctx.font_service.get_font_instance(
            self.text.font_handle.unwrap_or(ctx.default_font_handle()),
            self.text.font_size.unwrap_or(ctx.default_font_size()),
        );

        let rect_width = self.text.rect.width();
        let text_width = font_instance.compute_text_width(text, &mut ctx.texture_service);
        let cursor_width = font_instance.typical_advance_width();

        let selection_min_x = font_instance.compute_text_width(
            &text[..self.selection.cursor.start],
            &mut ctx.texture_service,
        );
        // TODO: don't recompute prefix width, sum prefix and "infix".
        let selection_max_x = font_instance
            .compute_text_width(&text[..self.selection.cursor.end], &mut ctx.texture_service);

        let mut scroll_x = self.selection.scroll_x;
        // right edge. scroll to show cursor + overscroll for cursor width.
        if selection_max_x + cursor_width - scroll_x > rect_width {
            scroll_x = selection_max_x + cursor_width - rect_width;
        }
        // left edge. scroll to show cursor.
        if selection_max_x < scroll_x {
            scroll_x = selection_max_x;
        }
        // undo overscroll when cursor moves back. if we can show all text without overscrolling,
        // do that.
        if selection_max_x + cursor_width <= text_width && text_width > rect_width {
            scroll_x = scroll_x.min(text_width - rect_width);
        }
        self.selection.scroll_x = scroll_x;

        if !self.selection.is_empty() {
            draw_singleline_text_selection(
                &self.text,
                selection_min_x,
                selection_max_x,
                self.selection.scroll_x,
                self.active,
                &mut font_instance,
                &mut ctx.draw_buffer,
            );
        }

        if self.active {
            let min = self.text.rect.min - Vec2::new(self.selection.scroll_x, 0.0)
                + Vec2::new(selection_max_x, 0.0);
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

// TODO: support different line break modes or whatever. current idea is break anywhere doesn't
// matter where; if next char can't fit on current line it must move to the next one.

fn should_break_line(ch: char, advance_width: f32, current_x: f32, rect: Rect) -> bool {
    if ch == '\n' {
        return true;
    }

    assert!(current_x >= rect.left());
    let will_overflow = current_x + advance_width - rect.left() > rect.width();
    will_overflow
}

// NOTE: don't advance if line break was cause by whitespace character (`\n` is considered a
// whitespace).
fn should_consume_post_line_break_char(ch: char) -> bool {
    ch.is_whitespace()
}

fn layout_row<E: Externs>(
    str: &str,
    start_byte: usize,
    rect: Rect,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> Range<usize> {
    let mut current_x: f32 = rect.left();
    let mut end_byte: usize = start_byte;

    for ch in (&str[start_byte..]).chars() {
        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);
        let advance_width = glyph.advance_width();

        if should_break_line(ch, advance_width, current_x, rect) {
            if should_consume_post_line_break_char(ch) {
                end_byte += ch.len_utf8();
            }
            return start_byte..end_byte;
        }

        current_x += advance_width;
        end_byte += ch.len_utf8();
    }

    start_byte..str.len()
}

#[test]
fn test_layout_row() {
    // NOTE: it's a pretty poor test that makes very heavy assumptions about the fact that we're
    // dealing with monospace font. it will not be correct with non-monospace font (although it
    // might pass).

    const CHARS_PER_ROW: usize = 16;

    let mut ctx = tests::create_context();
    let mut font_instance = ctx
        .font_service
        .get_font_instance(ctx.default_font_handle(), ctx.default_font_size());

    let haiku = "With no bamboo hat\nDoes the drizzle fall on me?\nWhat care I of that?";
    tests::assert_all_glyphs_have_equal_advance_width(
        haiku,
        font_instance.reborrow_mut(),
        &mut ctx.texture_service,
    );
    // NOTE: assertion above /\ ensures that the width below \/ matches the assumption.
    let width = font_instance.typical_advance_width() * CHARS_PER_ROW as f32;
    let rect = Rect::new(Vec2::ZERO, Vec2::new(width, f32::INFINITY));

    let mut last_row_range = 0..0;
    while last_row_range.end < haiku.len() {
        last_row_range = layout_row(
            haiku,
            last_row_range.end,
            rect,
            font_instance.reborrow_mut(),
            &mut ctx.texture_service,
        );
        // NOTE: a line may include invisible(/ chars that must not be rendered) at the end.
        let row = &haiku[last_row_range.clone()];
        assert!(row.trim().len() <= CHARS_PER_ROW);
    }
}

fn compute_multiline_text_height<E: Externs>(text: &Text, ctx: &mut Context<E>) -> f32 {
    let str = text.buffer.as_str();
    let mut font_instance = ctx.font_service.get_font_instance(
        text.font_handle.unwrap_or(ctx.default_font_handle()),
        text.font_size.unwrap_or(ctx.default_font_size()),
    );
    let font_height = font_instance.height();

    let mut line_count = 0;
    let mut last_row_range = 0..0;
    while last_row_range.end < str.len() {
        last_row_range = layout_row(
            str,
            last_row_range.end,
            text.rect,
            font_instance.reborrow_mut(),
            &mut ctx.texture_service,
        );
        line_count += 1;
    }
    line_count as f32 * font_height
}

fn locate_multiline_text_coord<E: Externs>(
    text: &Text,
    position: Vec2,
    ctx: &mut Context<E>,
) -> usize {
    // maybe pointer is above
    if position.y < text.rect.top() {
        return 0;
    }

    let str = text.buffer.as_str();
    let mut font_instance = ctx.font_service.get_font_instance(
        text.font_handle.unwrap_or(ctx.default_font_handle()),
        text.font_size.unwrap_or(ctx.default_font_size()),
    );
    let font_height = font_instance.height();

    let mut line_num = 0;
    let mut last_row_range = 0..0;
    while last_row_range.end < str.len() {
        last_row_range = layout_row(
            str,
            last_row_range.end,
            text.rect,
            font_instance.reborrow_mut(),
            &mut ctx.texture_service,
        );

        line_num += 1;

        // maybe this is the line
        let end_y = text.rect.top() + line_num as f32 * font_height;
        if position.y < end_y {
            break;
        }
    }

    // maybe pointer is below
    let end_y = text.rect.top() + line_num as f32 * font_height;
    if position.y > end_y {
        return str.len();
    }

    last_row_range.start
        + locate_singleline_text_coord(
            &str[last_row_range],
            text.rect.left(),
            position,
            font_instance,
            &mut ctx.texture_service,
        )
}

// ----
// draw multline stuff

// TODO: draw_multiline_text_selection should also draw cursor (if draw_cursor is on or something).
//
// TODO: rename draw_multiline_text_selection to draw_multiline_selection.
fn draw_multiline_text_selection<E: Externs>(
    text: &Text,
    selection: &TextSelection,
    active: bool,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
    draw_buffer: &mut DrawBuffer<E>,
) {
    let str = text.buffer.as_str();
    let selection_range = selection.normalized_cursor();
    let font_height = font_instance.height();
    let fill = if active {
        text.palette
            .as_ref()
            .map_or_else(|| SELECTION_ACTIVE, |a| a.selection_active)
    } else {
        text.palette
            .as_ref()
            .map_or_else(|| SELECTION_INACTIVE, |a| a.selection_inactive)
    };

    let mut line_num = 0;
    let mut last_row_range = 0..0;
    while last_row_range.end < str.len() {
        last_row_range = layout_row(
            str,
            last_row_range.end,
            text.rect,
            font_instance.reborrow_mut(),
            texture_service,
        );
        if selection_range.end < last_row_range.start || selection_range.start > last_row_range.end
        {
            // TODO: play around with the scope guard thing (aka defer).
            line_num += 1;
            continue;
        }

        let fragment_range = selection_range.start.max(last_row_range.start)
            ..selection_range.end.min(last_row_range.end);
        let relative_range =
            fragment_range.start - last_row_range.start..fragment_range.end - last_row_range.start;

        let row = &text.buffer.as_str()[last_row_range.clone()];
        let prefix = &row[..relative_range.start];
        let infix = &row[relative_range];

        let prefix_width = font_instance.compute_text_width(prefix, texture_service);
        let infix_width = font_instance.compute_text_width(infix, texture_service);

        let min_x = prefix_width;
        let max_x = prefix_width + infix_width;

        // TODO: min_ max_
        let min_y = text.rect.top() + line_num as f32 * font_height;
        let max_y = text.rect.top() + (line_num + 1) as f32 * font_height;

        let rect = Rect::new(
            Vec2::new(text.rect.left() + min_x, min_y),
            Vec2::new(text.rect.left() + max_x, max_y),
        );
        draw_buffer.push_rect(RectShape::with_fill(rect, Fill::with_color(fill)));

        line_num += 1;
    }
}

// TODO: y scroll or something. i want to be able to "scroll to bottom".
fn draw_multiline_text<E: Externs>(
    text: &Text,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
    draw_buffer: &mut DrawBuffer<E>,
) {
    let str = text.buffer.as_str();
    let fg = text.palette.as_ref().map(|a| a.fg).unwrap_or(FG);
    let font_ascent = font_instance.ascent();
    let font_height = font_instance.height();

    let mut position = text.rect.top_left();
    for ch in str.chars() {
        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);
        let advance_width = glyph.advance_width();

        if should_break_line(ch, advance_width, position.x, text.rect) {
            position.x = text.rect.left();
            position.y += font_height;

            if should_consume_post_line_break_char(ch) {
                continue;
            }
        }

        draw_buffer.push_rect(RectShape::with_fill(
            glyph
                .bounding_rect()
                .translate_by(&Vec2::new(position.x, position.y + font_ascent)),
            Fill::new(
                fg,
                FillTexture {
                    kind: TextureKind::Internal(glyph.tex_handle()),
                    coords: glyph.tex_coords(),
                },
            ),
        ));

        position.x += advance_width;
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
        // maybe not!
        // but also maybe there needs to be a param that would allow to specify minimum amount of
        // rows?
        let height = compute_multiline_text_height(&self.text, ctx);
        let size = Vec2::new(self.text.rect.width(), height);
        let interaction_rect = Rect::new(self.text.rect.min, self.text.rect.min + size);
        ctx.maybe_set_hot_or_active(key, interaction_rect, CursorShape::Text, input);
        self.hot = ctx.is_hot(key);
        self.active = ctx.is_active(key);

        self
    }

    pub fn update<E: Externs>(self, _key: Key, ctx: &mut Context<E>, input: &input::State) -> Self {
        let KeyboardState { ref scancodes, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowLeft,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    self.selection.move_left(self.text.buffer.as_str(), true);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowRight,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    self.selection.move_right(self.text.buffer.as_str(), true);
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::Home,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    self.selection.move_home(self.text.buffer.as_str(), true);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::End,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    self.selection.move_end(self.text.buffer.as_str(), true);
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::C,
                    ..
                }) if scancodes.any_pressed([Scancode::CtrlLeft, Scancode::CtrlRight]) => {
                    let text = self.text.buffer.as_str();
                    if let Some(copy) = self.selection.copy(text) {
                        // TODO: consider allocating copy into single-frame arena or something.
                        ctx.request_clipboard_write(copy.to_string());
                    }
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

        if !self.selection.is_empty() {
            draw_multiline_text_selection(
                &self.text,
                self.selection,
                self.active,
                font_instance.reborrow_mut(),
                &mut ctx.texture_service,
                &mut ctx.draw_buffer,
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    pub struct TestExterns;

    impl Externs for TestExterns {
        type TextureHandle = ();
    }

    pub fn create_context() -> Context<TestExterns> {
        Context::<TestExterns>::default()
    }

    pub fn assert_all_glyphs_have_equal_advance_width(
        text: &str,
        mut font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<TestExterns>,
    ) {
        let mut prev_advance_width: Option<f32> = None;
        for ch in text.chars() {
            let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);
            let advance_width = glyph.advance_width();
            if let Some(prev_advance_width) = prev_advance_width.replace(advance_width) {
                assert_eq!(prev_advance_width, advance_width);
            }
        }
    }
}
