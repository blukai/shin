use std::ops::{DerefMut, Range};

use input::{
    CursorShape, Event, KeyboardEvent, KeyboardState, Keycode, PointerButton, PointerEvent,
    Scancode,
};
use scopeguard::ScopeGuard;

use crate::{
    ClipboardState, Context, DrawBuffer, Externs, F64Vec2, Fill, FillTexture, FontHandle,
    FontInstanceRefMut, InteractionState, Key, Rect, RectShape, Rgba8, TextureKind, TextureService,
    Vec2,
};

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

// TODO: draw inactive cursor (maybe only outline?)

// TODO: support scrolling in non-selectable and non-editable text too. but input needs to support
// scrolling (mouse wheel / trackpad).

// TODO: draw scrollbars.

// TODO: try to introduce idea of z-indexes or something, some kind of layers, something that would
// allow to sort of push things into drawing queue, but put it behind. might also take into
// consideration idea of tooltips (which would be the opposite of behind).
//
// i want to be able to do "underlays". i want to be able to treat text selection as an underlay. i
// want to be able to specify custom underlays from outside that are different from text selection
// - for example diffs (for diff there would be an underlay for a like and for a subset of line's
// content).

// TODO: culling - don't draw stuff that is not within the clip rect.

// TODO: support different line break modes or whatever. current idea is break anywhere doesn't
// matter where; if next char can't fit on current line it must move to the next one.

// ----
// testing utils

#[cfg(test)]
mod tests {
    use crate::{Externs, FontInstanceRefMut, TextureService};

    pub fn assert_all_glyphs_have_equal_advance_width<E: Externs>(
        str: &str,
        mut font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<E>,
    ) {
        let mut prev_advance_width: Option<f32> = None;
        for ch in str.chars() {
            let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);
            let advance_width = glyph.advance_width();
            if let Some(prev_advance_width) = prev_advance_width.replace(advance_width) {
                assert_eq!(prev_advance_width, advance_width);
            }
        }
    }
}

// ----
// actual stuff

fn should_break_line(ch: char, advance_width: f32, current_x: f32, rect: Rect) -> bool {
    if ch == '\n' {
        return true;
    }

    assert!(current_x >= rect.min.x);
    let will_overflow = current_x + advance_width - rect.min.x > rect.width();
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
    let mut current_x: f32 = rect.min.x;
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

    use crate::UnitExterns;

    const CHARS_PER_ROW: usize = 16;

    let mut ctx = Context::<UnitExterns>::default();
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

fn count_rows<E: Externs>(
    str: &str,
    rect: Rect,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> usize {
    let mut line_count = 0;
    let mut last_row_range = 0..0;
    while last_row_range.end < str.len() {
        last_row_range = layout_row(
            str,
            last_row_range.end,
            rect,
            font_instance.reborrow_mut(),
            texture_service,
        );
        line_count += 1;
    }
    line_count
}

// returns byte offset(not char index)
fn locate_singleline_coord<E: Externs>(
    str: &str,
    container_rect: Rect,
    scroll_offset: Vec2,
    coord: Vec2,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> usize {
    let left = container_rect.min.x - scroll_offset.x;
    if coord.x < left {
        return 0;
    }

    let mut byte_offset: usize = 0;
    let mut x_offset: f32 = left;
    for ch in str.chars() {
        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);

        let min_x = x_offset;
        let max_x = min_x + glyph.advance_width();
        if coord.x >= min_x && coord.x <= max_x {
            // NOTE: it seems like everyone consider char selected only if you're reaching past
            // half of it.
            let center_x = min_x + (max_x - min_x) / 2.0;
            if coord.x < center_x {
                return byte_offset;
            } else {
                return byte_offset + ch.len_utf8();
            }
        }

        byte_offset += ch.len_utf8();
        x_offset += glyph.advance_width();
    }

    // the pointer is after end of the line.
    assert!(coord.x > x_offset);
    str.len()
}

// returns byte offset(not char index)
fn locate_multiline_coord<E: Externs>(
    str: &str,
    container_rect: Rect,
    scroll_offset: Vec2,
    coord: Vec2,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> usize {
    let top = container_rect.min.y - scroll_offset.y;
    if coord.y < top {
        return 0;
    }

    let font_height = font_instance.height();

    let mut line_num = 0;
    let mut last_row_range = 0..0;
    while last_row_range.end < str.len() {
        last_row_range = layout_row(
            str,
            last_row_range.end,
            container_rect,
            font_instance.reborrow_mut(),
            texture_service,
        );

        line_num += 1;

        // maybe this is the line
        let max_y = top + line_num as f32 * font_height;
        if coord.y < max_y {
            break;
        }
    }

    // maybe pointer is below
    let max_y = top + line_num as f32 * font_height;
    if coord.y > max_y {
        return str.len();
    }

    let offset_within_row = locate_singleline_coord(
        &str[last_row_range.clone()],
        container_rect,
        scroll_offset,
        coord,
        font_instance,
        texture_service,
    );
    last_row_range.start + offset_within_row
}

fn scroll_into_singleline_cursor<E: Externs>(
    str: &str,
    cursor_byte_range: Range<usize>,
    container_rect: Rect,
    scroll_offset: Vec2,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> Vec2 {
    let container_width = container_rect.width();
    let cursor_width = font_instance.typical_advance_width();

    let pre_cursor_text = &str[..cursor_byte_range.end];
    let post_cursor_text = &str[cursor_byte_range.end..];

    let pre_cursor_text_width = font_instance.compute_text_width(pre_cursor_text, texture_service);
    let post_cursor_text_width =
        font_instance.compute_text_width(post_cursor_text, texture_service);
    let text_width = pre_cursor_text_width + post_cursor_text_width;

    let cursor_min_x = pre_cursor_text_width;
    let cursor_max_x = cursor_min_x + cursor_width;

    // right edge. scroll to show cursor + overscroll for cursor width.
    if cursor_max_x - scroll_offset.x > container_width {
        return scroll_offset.with_x(cursor_max_x - container_width);
    }

    // left edge. scroll to show cursor.
    if cursor_min_x < scroll_offset.x {
        return scroll_offset.with_x(cursor_min_x);
    }

    // undo overscroll when cursor moves back. if we can show all text without overscrolling, do
    // that.
    if cursor_max_x <= text_width && text_width > container_width {
        return scroll_offset.with_x(scroll_offset.x.min(text_width - container_width));
    }

    scroll_offset
}

// TODO: scroll_into_multiline_cursor need to support different row wrapping modes.
fn scroll_into_multiline_cursor<E: Externs>(
    str: &str,
    cursor_byte_range: Range<usize>,
    container_rect: Rect,
    scroll_offset: Vec2,
    font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> Vec2 {
    let container_height = container_rect.height();
    let font_height = font_instance.height();

    let pre_cursor_text = &str[..cursor_byte_range.end];
    let pre_cursor_row_count = count_rows(
        pre_cursor_text,
        container_rect,
        font_instance,
        texture_service,
    );

    let cursor_max_y = pre_cursor_row_count as f32 * font_height;
    let cursor_min_y = (cursor_max_y - font_height).max(0.0);

    // bottom edge
    if cursor_max_y - scroll_offset.y > container_height {
        return scroll_offset.with_y(cursor_max_y - container_height);
    }
    // top edge
    if cursor_min_y < scroll_offset.y {
        return scroll_offset.with_y(cursor_min_y);
    }

    scroll_offset
}

// ----
// draw singleline

// TODO: draw_singleline_selection sucks. need some kind of z-indices in draw buffer to be able to
// draw stuff iteratively and "commit" it different layers.
fn draw_singleline_selection<E: Externs>(
    text: &Text,
    state: &TextState,
    active: bool,
    draw_cursor: bool,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
    draw_buffer: &mut DrawBuffer<E>,
) {
    // early bail out
    if state.selection.is_empty() && !draw_cursor {
        return;
    }

    let str = text.buffer.as_str();
    let normalized_selection_byte_range = state.selection.normalized_byte_range();
    let pre_selection_text = &str[..normalized_selection_byte_range.start];
    let selection_text = &str[normalized_selection_byte_range];

    let pre_selection_text_width =
        font_instance.compute_text_width(pre_selection_text, texture_service);
    let selection_text_width = font_instance.compute_text_width(selection_text, texture_service);

    let selection_min_x = pre_selection_text_width;
    let selection_max_x = pre_selection_text_width + selection_text_width;

    // NOTE: end is where the cursor is. for example in `hello, sailor` selection may have started
    // at `,` and moved left to `e`.
    if !state.selection.is_empty() {
        let selection_width = selection_max_x - selection_min_x;

        let min =
            text.rect.min - Vec2::new(state.scroll.offset.x, 0.0) + Vec2::new(selection_min_x, 0.0);
        let size = Vec2::new(selection_width, font_instance.height());
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
        draw_buffer.push_rect(RectShape::new_with_fill(rect, Fill::new_with_color(fill)));
    }

    // TODO: draw inactive cursor, maybe outlined or something.
    if draw_cursor && active {
        let cursor_min_x = if state.selection.is_byte_range_normalized() {
            selection_max_x
        } else {
            selection_min_x
        };
        let cursor_width = font_instance.typical_advance_width();

        let min =
            text.rect.min - Vec2::new(state.scroll.offset.x, 0.0) + Vec2::new(cursor_min_x, 0.0);
        let size = Vec2::new(cursor_width, font_instance.height());
        let rect = Rect::new(min, min + size);
        let fill = text.palette.as_ref().map_or_else(|| CURSOR, |a| a.cursor);
        draw_buffer.push_rect(RectShape::new_with_fill(rect, Fill::new_with_color(fill)));
    }
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

        draw_buffer.push_rect(RectShape::new_with_fill(
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
// draw multiline

// TODO: draw_multiline_text_selection should also draw cursor (if draw_cursor is on or something).
fn draw_multiline_selection<E: Externs>(
    text: &Text,
    state: &TextState,
    active: bool,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
    draw_buffer: &mut DrawBuffer<E>,
) {
    let str = text.buffer.as_str();
    let top = text.rect.min.y - state.scroll.offset.y;
    let selection_range = state.selection.normalized_byte_range();
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
        let line_num = ScopeGuard::new_with_data(line_num, |_| line_num += 1);

        last_row_range = layout_row(
            str,
            last_row_range.end,
            text.rect,
            font_instance.reborrow_mut(),
            texture_service,
        );
        if selection_range.end < last_row_range.start || selection_range.start > last_row_range.end
        {
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

        let min_y = top + *line_num as f32 * font_height;
        let max_y = top + *line_num as f32 * font_height + font_height;

        let rect = Rect::new(
            Vec2::new(text.rect.min.x + min_x, min_y),
            Vec2::new(text.rect.min.x + max_x, max_y),
        );
        draw_buffer.push_rect(RectShape::new_with_fill(rect, Fill::new_with_color(fill)));
    }
}

// TODO: y scroll or something. i want to be able to "scroll to bottom".
fn draw_multiline_text<E: Externs>(
    text: &Text,
    scroll_y: f32,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
    draw_buffer: &mut DrawBuffer<E>,
) {
    let str = text.buffer.as_str();
    let fg = text.palette.as_ref().map(|a| a.fg).unwrap_or(FG);
    let font_ascent = font_instance.ascent();
    let font_height = font_instance.height();

    let mut position = text.rect.top_left();
    position.y -= scroll_y;
    for ch in str.chars() {
        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);
        let advance_width = glyph.advance_width();

        if should_break_line(ch, advance_width, position.x, text.rect) {
            position.x = text.rect.min.x;
            position.y += font_height;

            if should_consume_post_line_break_char(ch) {
                continue;
            }
        }

        draw_buffer.push_rect(RectShape::new_with_fill(
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
struct TextSelection {
    /// - if empty (start == end) -> no selection.
    /// - start may be less than or greater than end.
    /// - start is where the initial click was.
    /// - end is where the cursor is.
    byte_range: Range<usize>,
}

impl TextSelection {
    fn is_empty(&self) -> bool {
        self.byte_range.start == self.byte_range.end
    }

    fn clear(&mut self) {
        self.byte_range = 0..0;
    }

    fn normalized_byte_range(&self) -> Range<usize> {
        let min = self.byte_range.start.min(self.byte_range.end);
        let max = self.byte_range.start.max(self.byte_range.end);
        min..max
    }

    fn is_byte_range_normalized(&self) -> bool {
        self.byte_range.start <= self.byte_range.end
    }

    // TODO: move modifiers (by char, by char type, by word, etc.)

    fn move_left(&mut self, text: &str, extend_selection: bool) {
        if !self.is_empty() && !extend_selection {
            self.byte_range.end = self.byte_range.end.min(self.byte_range.start);
            self.byte_range.start = self.byte_range.end;
            return;
        }

        let prev_char_width = &text[..self.byte_range.end]
            .chars()
            .next_back()
            .map_or_else(|| 0, |ch| ch.len_utf8());
        self.byte_range.end -= prev_char_width;
        if !extend_selection {
            self.byte_range.start = self.byte_range.end;
        }
    }

    fn move_right(&mut self, text: &str, extend_selection: bool) {
        if !self.is_empty() && !extend_selection {
            self.byte_range.end = self.byte_range.end.max(self.byte_range.start);
            self.byte_range.start = self.byte_range.end;
            return;
        }

        let next_char_width = &text[self.byte_range.end..]
            .chars()
            .next()
            .map_or_else(|| 0, |ch| ch.len_utf8());
        self.byte_range.end += next_char_width;
        if !extend_selection {
            self.byte_range.start = self.byte_range.end;
        }
    }

    fn move_home(&mut self, text: &str, extend_selection: bool) {
        self.byte_range.end = text[..self.byte_range.end]
            .rfind('\n')
            .map_or_else(|| 0, |i| i + 1);
        if !extend_selection {
            self.byte_range.start = self.byte_range.end;
        }
    }

    fn move_end(&mut self, text: &str, extend_selection: bool) {
        self.byte_range.end = text[self.byte_range.end..]
            .find('\n')
            .map_or_else(|| text.len(), |i| self.byte_range.end + i);
        if !extend_selection {
            self.byte_range.start = self.byte_range.end;
        }
    }

    fn delete_selection(&mut self, text: &mut String) {
        let normalized_byte_range = self.normalized_byte_range();
        if normalized_byte_range.end > normalized_byte_range.start {
            text.replace_range(normalized_byte_range, "");
        }
        self.byte_range.end = self.byte_range.end.min(self.byte_range.start);
        self.byte_range.start = self.byte_range.end;
    }

    fn delete_left(&mut self, text: &mut String) {
        if self.is_empty() {
            self.byte_range.end = self.byte_range.start;
            self.move_left(text, true);
        }
        self.delete_selection(text);
    }

    fn delete_right(&mut self, text: &mut String) {
        if self.is_empty() {
            self.byte_range.end = self.byte_range.start;
            self.move_right(text, true);
        }
        self.delete_selection(text);
    }

    fn insert_char(&mut self, text: &mut String, ch: char) {
        if !self.is_empty() {
            self.delete_selection(text);
        }
        assert_eq!(self.byte_range.start, self.byte_range.end);
        text.insert(self.byte_range.start, ch);
        self.byte_range.start += ch.len_utf8();
        self.byte_range.end = self.byte_range.start;
    }

    fn paste(&mut self, text: &mut String, pasta: &str) {
        let normalized_byte_range = self.normalized_byte_range();
        if self.is_empty() {
            text.insert_str(normalized_byte_range.start, pasta);
        } else {
            text.replace_range(normalized_byte_range.clone(), pasta);
        }
        self.byte_range.end = normalized_byte_range.start + pasta.len();
        self.byte_range.start = self.byte_range.end;
    }

    fn copy<'a>(&self, text: &'a str) -> Option<&'a str> {
        if self.is_empty() {
            return None;
        }
        let normalized_byte_range = self.normalized_byte_range();
        Some(&text[normalized_byte_range])
    }
}

// TODO: consider animating scroll.
//
// TODO: maybe try to generalize Animation (from console example).
#[derive(Default)]
struct TextScroll {
    offset: Vec2,
}

impl TextScroll {
    fn clear(&mut self) {
        self.offset = Vec2::default();
    }
}

#[derive(Default)]
pub struct TextState {
    selection: TextSelection,
    scroll: TextScroll,
}

impl TextState {
    pub fn clear(&mut self) {
        self.selection.clear();
        self.scroll.clear();
    }
}

// TODO: get rid of TextBuffer. supply text in final "build" call.
//
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

// ----

pub struct Text<'a> {
    key: Key,
    buffer: TextBuffer<'a>,
    rect: Rect,

    font_handle: Option<FontHandle>,
    font_size: Option<f32>,
    palette: Option<TextPalette>,

    // NOTE: this is also sort of "state", but the kind that doesn't need to survive across frames.
    hot: Option<bool>,
    active: Option<bool>,
}

impl<'a> Text<'a> {
    #[track_caller]
    pub fn new<B: Into<TextBuffer<'a>>>(text: B, rect: Rect) -> Self {
        Self {
            key: Key::from_location(std::panic::Location::caller()),
            buffer: text.into(),
            rect,

            font_handle: None,
            font_size: None,
            palette: None,

            hot: None,
            active: None,
        }
    }

    /// you should provide custom key when rendering stuff in a loop (maybe use
    /// [`Key::from_caller_location_and`]).
    pub fn with_key(mut self, key: Key) -> Self {
        self.key = key;
        self
    }

    pub fn with_font_handle(mut self, value: FontHandle) -> Self {
        self.font_handle = Some(value);
        self
    }

    pub fn with_font_size(mut self, value: f32) -> Self {
        self.font_size = Some(value);
        self
    }

    // TODO: this sucks. i don't want to supply a full palette to just change foreground color.
    pub fn with_palette(mut self, value: TextPalette) -> Self {
        self.palette = Some(value);
        self
    }

    /// you should provide `hot` and `active` values if you did invoke
    /// [`InteractionState::maybe_set_hot_or_active`] manually for this widget.
    pub fn with_maybe_hot_or_active(mut self, hot: bool, active: bool) -> Self {
        self.hot.replace(hot);
        self.active.replace(active);
        self
    }

    pub fn singleline(self) -> TextSingleline<'a> {
        TextSingleline::new(self)
    }

    pub fn multiline(self) -> TextMultiline<'a> {
        TextMultiline::new(self)
    }

    // ----

    #[allow(dead_code)]
    fn is_hot(&self) -> bool {
        self.hot == Some(true)
    }

    fn is_active(&self) -> bool {
        self.active == Some(true)
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
        let font_instance = ctx.font_service.get_font_instance(
            self.text.font_handle.unwrap_or(ctx.default_font_handle()),
            self.text.font_size.unwrap_or(ctx.default_font_size()),
        );
        let mut draw_buffer = ctx.draw_buffer.clip_scope(self.text.rect);

        draw_singleline_text(
            &self.text,
            0.0,
            font_instance,
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );
    }

    pub fn selectable(self, state: &'a mut TextState) -> TextSinglelineSelectable<'a> {
        TextSinglelineSelectable::new(self.text, state)
    }

    pub fn editable(self, state: &'a mut TextState) -> TextSinglelineEditable<'a> {
        TextSinglelineEditable::new(self.text, state)
    }
}

pub struct TextSinglelineSelectable<'a> {
    text: Text<'a>,
    state: &'a mut TextState,
}

impl<'a> TextSinglelineSelectable<'a> {
    fn new(text: Text<'a>, state: &'a mut TextState) -> Self {
        Self { text, state }
    }

    // TODO: get rid of maybe_set_hot_or_active
    fn maybe_set_hot_or_active<E: Externs>(
        &mut self,
        mut font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<E>,
        interaction_state: &mut InteractionState,
        input: &input::State,
    ) {
        let height = font_instance.height();
        let width = font_instance.compute_text_width(self.text.buffer.as_str(), texture_service);
        let size = Vec2::new(width, height);
        let interaction_rect = Rect::new(self.text.rect.min, self.text.rect.min + size);

        interaction_state.maybe_set_hot_or_active(
            self.text.key,
            interaction_rect,
            CursorShape::Text,
            input,
        );
        self.text
            .hot
            .replace(interaction_state.is_hot(self.text.key));
        self.text
            .active
            .replace(interaction_state.is_active(self.text.key));
    }

    fn update<E: Externs>(
        &mut self,
        mut font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<E>,
        interaction_state: &mut InteractionState,
        clipboard_state: &mut ClipboardState,
        input: &input::State,
    ) {
        if self.text.hot.is_none() && self.text.active.is_none() {
            self.maybe_set_hot_or_active(
                font_instance.reborrow_mut(),
                texture_service,
                interaction_state,
                input,
            );
        }
        if !self.text.is_active() {
            return;
        }

        let KeyboardState { ref keymods, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowLeft,
                    ..
                }) if keymods.shift() => {
                    self.state
                        .selection
                        .move_left(self.text.buffer.as_str(), true);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowRight,
                    ..
                }) if keymods.shift() => {
                    self.state
                        .selection
                        .move_right(self.text.buffer.as_str(), true);
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::Home,
                    ..
                }) if keymods.shift() => {
                    self.state
                        .selection
                        .move_home(self.text.buffer.as_str(), true);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::End,
                    ..
                }) if keymods.shift() => {
                    self.state
                        .selection
                        .move_end(self.text.buffer.as_str(), true);
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::C,
                    ..
                }) if keymods.ctrl() => {
                    let str = self.text.buffer.as_str();
                    if let Some(copy) = self.state.selection.copy(str) {
                        // TODO: consider allocating copy into single-frame arena or something.
                        clipboard_state.request_write(copy.to_string());
                    }
                }

                Event::Pointer(
                    pe @ PointerEvent::Press {
                        button: PointerButton::Primary,
                    },
                )
                | Event::Pointer(pe @ PointerEvent::Motion { .. })
                    if input
                        .pointer
                        .press_origins
                        .get(&PointerButton::Primary)
                        .is_some_and(|p| {
                            self.text.rect.contains(&Vec2::from(F64Vec2::from(*p)))
                        }) =>
                {
                    let byte_offset = locate_singleline_coord(
                        self.text.buffer.as_str(),
                        self.text.rect,
                        self.state.scroll.offset,
                        Vec2::from(F64Vec2::from(input.pointer.position)),
                        font_instance.reborrow_mut(),
                        texture_service,
                    );
                    if let PointerEvent::Press { .. } = pe {
                        self.state.selection.byte_range = byte_offset..byte_offset;
                    } else {
                        self.state.selection.byte_range.end = byte_offset;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn draw<E: Externs>(mut self, ctx: &mut Context<E>, input: &input::State) {
        let mut font_instance = ctx.font_service.get_font_instance(
            self.text.font_handle.unwrap_or(ctx.default_font_handle()),
            self.text.font_size.unwrap_or(ctx.default_font_size()),
        );
        let mut draw_buffer = ctx.draw_buffer.clip_scope(self.text.rect);

        self.update(
            font_instance.reborrow_mut(),
            &mut ctx.texture_service,
            &mut ctx.interaction_state,
            &mut ctx.clipboard_state,
            input,
        );

        draw_singleline_selection(
            &self.text,
            self.state,
            self.text.is_active(),
            false,
            font_instance.reborrow_mut(),
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );

        draw_singleline_text(
            &self.text,
            0.0,
            font_instance,
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );
    }
}

pub struct TextSinglelineEditable<'a> {
    text: Text<'a>,
    state: &'a mut TextState,
}

impl<'a> TextSinglelineEditable<'a> {
    fn new(text: Text<'a>, state: &'a mut TextState) -> Self {
        Self { text, state }
    }

    // TODO: get rid of maybe_set_hot_or_active
    fn maybe_set_hot_or_active<E: Externs>(
        &mut self,
        mut font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<E>,
        interaction_state: &mut InteractionState,
        input: &input::State,
    ) {
        let height = font_instance.height();
        let width = font_instance.compute_text_width(self.text.buffer.as_str(), texture_service);
        let size = Vec2::new(width, height);
        let interaction_rect = Rect::new(self.text.rect.min, self.text.rect.min + size);

        interaction_state.maybe_set_hot_or_active(
            self.text.key,
            interaction_rect,
            CursorShape::Text,
            input,
        );
        self.text
            .hot
            .replace(interaction_state.is_hot(self.text.key));
        self.text
            .active
            .replace(interaction_state.is_active(self.text.key));
    }

    fn update<E: Externs>(
        &mut self,
        mut font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<E>,
        interaction_state: &mut InteractionState,
        clipboard_state: &mut ClipboardState,
        input: &input::State,
    ) {
        if self.text.hot.is_none() && self.text.active.is_none() {
            self.maybe_set_hot_or_active(
                font_instance.reborrow_mut(),
                texture_service,
                interaction_state,
                input,
            );
        }
        if !self.text.is_active() {
            return;
        }

        let KeyboardState { ref keymods, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowLeft,
                    ..
                }) => {
                    self.state
                        .selection
                        .move_left(self.text.buffer.as_str(), keymods.shift());
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowRight,
                    ..
                }) => {
                    self.state
                        .selection
                        .move_right(self.text.buffer.as_str(), keymods.shift());
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::Home,
                    ..
                }) => {
                    self.state
                        .selection
                        .move_home(self.text.buffer.as_str(), keymods.shift());
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::End,
                    ..
                }) => {
                    self.state
                        .selection
                        .move_end(self.text.buffer.as_str(), keymods.shift());
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::V,
                    ..
                }) if keymods.ctrl() => {
                    clipboard_state.request_read(self.text.key);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::C,
                    ..
                }) if keymods.ctrl() => {
                    let str = self.text.buffer.as_string_mut().unwrap();
                    if let Some(copy) = self.state.selection.copy(str) {
                        // TODO: consider allocating copy into single-frame arena or something.
                        clipboard_state.request_write(copy.to_string());
                    }
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::X,
                    ..
                }) if keymods.ctrl() => {
                    let str = self.text.buffer.as_string_mut().unwrap();
                    if let Some(copy) = self.state.selection.copy(str) {
                        // TODO: consider allocating copy into single-frame arena or something.
                        clipboard_state.request_write(copy.to_string());
                        self.state.selection.delete_selection(str);
                    }
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::Backspace,
                    ..
                }) => {
                    self.state
                        .selection
                        .delete_left(self.text.buffer.as_string_mut().unwrap());
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::Delete,
                    ..
                }) => {
                    self.state
                        .selection
                        .delete_right(self.text.buffer.as_string_mut().unwrap());
                }
                Event::Keyboard(KeyboardEvent::Press {
                    keycode: Keycode::Char(ch),
                    ..
                }) if *ch as u32 >= 32 && *ch as u32 != 127 => {
                    // TODO: maybe better printability check ^.
                    self.state
                        .selection
                        .insert_char(self.text.buffer.as_string_mut().unwrap(), *ch);
                }

                Event::Pointer(
                    ev @ PointerEvent::Press {
                        button: PointerButton::Primary,
                    },
                )
                | Event::Pointer(ev @ PointerEvent::Motion { .. })
                    if input
                        .pointer
                        .press_origins
                        .get(&PointerButton::Primary)
                        .is_some_and(|p| {
                            self.text.rect.contains(&Vec2::from(F64Vec2::from(*p)))
                        }) =>
                {
                    let byte_offset = locate_singleline_coord(
                        self.text.buffer.as_str(),
                        self.text.rect,
                        self.state.scroll.offset,
                        Vec2::from(F64Vec2::from(input.pointer.position)),
                        font_instance.reborrow_mut(),
                        texture_service,
                    );
                    if let PointerEvent::Press { .. } = ev {
                        self.state.selection.byte_range = byte_offset..byte_offset;
                    } else {
                        self.state.selection.byte_range.end = byte_offset;
                    }
                }
                _ => {}
            }
        }

        if let Some(pasta) = clipboard_state.try_take_read(self.text.key) {
            // TODO: consider removing line breaks or something.
            self.state
                .selection
                .paste(self.text.buffer.as_string_mut().unwrap(), pasta.as_str());
        }

        // NOTE: cursor must be updated in reaction to possible interactions (above ^).
        self.state.scroll.offset = scroll_into_singleline_cursor(
            self.text.buffer.as_str(),
            self.state.selection.byte_range.clone(),
            self.text.rect,
            self.state.scroll.offset,
            font_instance,
            texture_service,
        );
    }

    pub fn draw<E: Externs>(mut self, ctx: &mut Context<E>, input: &input::State) {
        let mut font_instance = ctx.font_service.get_font_instance(
            self.text.font_handle.unwrap_or(ctx.default_font_handle()),
            self.text.font_size.unwrap_or(ctx.default_font_size()),
        );
        let mut draw_buffer = ctx.draw_buffer.clip_scope(self.text.rect);

        self.update(
            font_instance.reborrow_mut(),
            &mut ctx.texture_service,
            &mut ctx.interaction_state,
            &mut ctx.clipboard_state,
            input,
        );

        draw_singleline_selection(
            &self.text,
            self.state,
            self.text.is_active(),
            true,
            font_instance.reborrow_mut(),
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );

        draw_singleline_text(
            &self.text,
            self.state.scroll.offset.x,
            font_instance,
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );
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
        let font_instance = ctx.font_service.get_font_instance(
            self.text.font_handle.unwrap_or(ctx.default_font_handle()),
            self.text.font_size.unwrap_or(ctx.default_font_size()),
        );
        let mut draw_buffer = ctx.draw_buffer.clip_scope(self.text.rect);

        draw_multiline_text(
            &self.text,
            0.0,
            font_instance,
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );
    }

    pub fn selectable(self, state: &'a mut TextState) -> TextMultilineSelectable<'a> {
        TextMultilineSelectable::new(self.text, state)
    }
}

pub struct TextMultilineSelectable<'a> {
    text: Text<'a>,
    state: &'a mut TextState,
}

impl<'a> TextMultilineSelectable<'a> {
    fn new(text: Text<'a>, state: &'a mut TextState) -> Self {
        Self { text, state }
    }

    // TODO: get rid of maybe_set_hot_or_active
    fn maybe_set_hot_or_active<E: Externs>(
        &mut self,
        font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<E>,
        interaction_state: &mut InteractionState,
        input: &input::State,
    ) {
        // TODO: do i need to compute multiline text height here really? wouldn't it make make
        // sense for the "text area" to reserve the entirety of available space?
        // maybe not!
        // but also maybe there needs to be a param that would allow to specify minimum amount of
        // rows?

        let font_height = font_instance.height();
        let row_count = count_rows(
            self.text.buffer.as_str(),
            self.text.rect,
            font_instance,
            texture_service,
        );
        let height = row_count as f32 * font_height;
        let size = Vec2::new(self.text.rect.width(), height);

        let interaction_rect = Rect::new(self.text.rect.min, self.text.rect.min + size);
        interaction_state.maybe_set_hot_or_active(
            self.text.key,
            interaction_rect,
            CursorShape::Text,
            input,
        );
        self.text
            .hot
            .replace(interaction_state.is_hot(self.text.key));
        self.text
            .active
            .replace(interaction_state.is_active(self.text.key));
    }

    fn update<E: Externs>(
        &mut self,
        mut font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<E>,
        interaction_state: &mut InteractionState,
        clipboard_state: &mut ClipboardState,
        input: &input::State,
    ) {
        if self.text.hot.is_none() && self.text.active.is_none() {
            self.maybe_set_hot_or_active(
                font_instance.reborrow_mut(),
                texture_service,
                interaction_state,
                input,
            );
        }
        if !self.text.is_active() {
            return;
        }

        let KeyboardState { ref keymods, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowLeft,
                    ..
                }) if keymods.shift() => {
                    self.state
                        .selection
                        .move_left(self.text.buffer.as_str(), true);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowRight,
                    ..
                }) if keymods.shift() => {
                    self.state
                        .selection
                        .move_right(self.text.buffer.as_str(), true);
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::Home,
                    ..
                }) if keymods.shift() => {
                    self.state
                        .selection
                        .move_home(self.text.buffer.as_str(), true);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::End,
                    ..
                }) if keymods.shift() => {
                    self.state
                        .selection
                        .move_end(self.text.buffer.as_str(), true);
                }

                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::C,
                    ..
                }) if keymods.ctrl() => {
                    let str = self.text.buffer.as_str();
                    if let Some(copy) = self.state.selection.copy(str) {
                        // TODO: consider allocating copy into single-frame arena or something.
                        clipboard_state.request_write(copy.to_string());
                    }
                }

                Event::Pointer(
                    pe @ PointerEvent::Press {
                        button: PointerButton::Primary,
                    },
                )
                | Event::Pointer(pe @ PointerEvent::Motion { .. })
                    if input
                        .pointer
                        .press_origins
                        .get(&PointerButton::Primary)
                        .is_some_and(|p| {
                            self.text.rect.contains(&Vec2::from(F64Vec2::from(*p)))
                        }) =>
                {
                    let byte_offset = locate_multiline_coord(
                        self.text.buffer.as_str(),
                        self.text.rect,
                        self.state.scroll.offset,
                        Vec2::from(F64Vec2::from(input.pointer.position)),
                        font_instance.reborrow_mut(),
                        texture_service,
                    );
                    if let PointerEvent::Press { .. } = pe {
                        self.state.selection.byte_range = byte_offset..byte_offset;
                    } else {
                        self.state.selection.byte_range.end = byte_offset;
                    }
                }
                _ => {}
            }
        }

        // NOTE: cursor must be updated in reaction to possible interactions (above ^).
        self.state.scroll.offset = scroll_into_multiline_cursor(
            self.text.buffer.as_str(),
            self.state.selection.byte_range.clone(),
            self.text.rect,
            self.state.scroll.offset,
            font_instance,
            texture_service,
        );
    }

    pub fn draw<E: Externs>(mut self, ctx: &mut Context<E>, input: &input::State) {
        let mut font_instance = ctx.font_service.get_font_instance(
            self.text.font_handle.unwrap_or(ctx.default_font_handle()),
            self.text.font_size.unwrap_or(ctx.default_font_size()),
        );
        let mut draw_buffer = ctx.draw_buffer.clip_scope(self.text.rect);

        self.update(
            font_instance.reborrow_mut(),
            &mut ctx.texture_service,
            &mut ctx.interaction_state,
            &mut ctx.clipboard_state,
            input,
        );

        draw_multiline_selection(
            &self.text,
            self.state,
            self.text.is_active(),
            font_instance.reborrow_mut(),
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );

        draw_multiline_text(
            &self.text,
            self.state.scroll.offset.y,
            font_instance,
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );
    }
}
