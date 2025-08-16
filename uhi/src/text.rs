use std::marker::PhantomData;
use std::ops::{DerefMut, Range};

use input::{
    Button, ButtonState, CursorShape, Event, KeyState, KeyboardEvent, KeyboardState, Keycode,
    PointerEvent, Scancode,
};

use crate::{
    Appearance, ClipboardState, Context, DrawBuffer, Externs, F64Vec2, Fill, FillTexture,
    FontHandle, FontInstanceRefMut, InteractionState, Key, Rect, RectShape, Rgba8, TextureKind,
    TextureService, Vec2,
};

// TODO: per-char layout styling
// - should be able to make some fragments of text bold?
// - should be able to change some elements of palette (fg, etc.)

// TODO: filters / input types
// - for example number-only input, etc.

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

// TODO: auto scroll to bottom for multliline text (/stick to bottom).

// TODO: culling - don't draw stuff that is not within the clip rect.

// TODO: support different line break modes or whatever. current idea is break anywhere doesn't
// matter where; if next char can't fit on current line it must move to the next one.

// TODO: undo/redo

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

#[derive(Clone)]
pub struct TextAppearance {
    pub font_handle: FontHandle,
    pub font_size: f32,

    pub fg: Rgba8,
    pub selection_active_bg: Rgba8,
    pub selection_inactive_bg: Rgba8,
    pub cursor_bg: Rgba8,
}

impl TextAppearance {
    pub fn from_appearance(appearance: &Appearance) -> Self {
        Self {
            font_handle: appearance.font_handle,
            font_size: appearance.font_size,

            fg: appearance.fg,
            selection_active_bg: appearance.selection_active_bg,
            selection_inactive_bg: appearance.selection_inactive_bg,
            cursor_bg: appearance.cursor_bg,
        }
    }

    pub fn with_font_size(mut self, value: f32) -> Self {
        self.font_size = value;
        self
    }

    pub fn with_fg(mut self, value: Rgba8) -> Self {
        self.fg = value;
        self
    }

    pub fn selection_bg(&self, active: bool) -> Rgba8 {
        if active {
            self.selection_active_bg
        } else {
            self.selection_inactive_bg
        }
    }
}

/// - empty if start == end -> no selection.
/// - start may be less than or greater than end (non-normalized).
/// - start is where the initial click was.
/// - end is where the cursor currently is.
#[derive(Debug, Default, Clone, Copy)]
struct TextSelection {
    start: usize,
    end: usize,
}

impl TextSelection {
    fn from_range(byte_range: Range<usize>) -> Self {
        Self {
            start: byte_range.start,
            end: byte_range.end,
        }
    }

    fn as_range(&self) -> Range<usize> {
        self.start..self.end
    }

    fn is_empty(&self) -> bool {
        self.start == self.end
    }

    fn clear(&mut self) {
        self.start = 0;
        self.end = 0;
    }

    fn normalized(&self) -> Self {
        Self::from_range(self.start.min(self.end)..self.start.max(self.end))
    }

    // TODO: move modifiers (by char, by char type, by word, etc.)

    fn move_left(&mut self, text: &str, extend_selection: bool) {
        if !self.is_empty() && !extend_selection {
            self.end = self.end.min(self.start);
            self.start = self.end;
            return;
        }

        let prev_char_width = &text[..self.end]
            .chars()
            .next_back()
            .map_or_else(|| 0, |ch| ch.len_utf8());
        self.end -= prev_char_width;
        if !extend_selection {
            self.start = self.end;
        }
    }

    fn move_right(&mut self, text: &str, extend_selection: bool) {
        if !self.is_empty() && !extend_selection {
            self.end = self.end.max(self.start);
            self.start = self.end;
            return;
        }

        let next_char_width = &text[self.end..]
            .chars()
            .next()
            .map_or_else(|| 0, |ch| ch.len_utf8());
        self.end += next_char_width;
        if !extend_selection {
            self.start = self.end;
        }
    }

    fn move_home(&mut self, text: &str, extend_selection: bool) {
        self.end = text[..self.end].rfind('\n').map_or_else(|| 0, |i| i + 1);
        if !extend_selection {
            self.start = self.end;
        }
    }

    fn move_end(&mut self, text: &str, extend_selection: bool) {
        self.end = text[self.end..]
            .find('\n')
            .map_or_else(|| text.len(), |i| self.end + i);
        if !extend_selection {
            self.start = self.end;
        }
    }

    // TODO: test this, make sure it's correct.
    fn delete_selection(&mut self, text: &mut String) {
        if self.is_empty() {
            return;
        }
        let normalized = self.normalized();
        text.replace_range(normalized.as_range(), "");
        self.end = normalized.start;
        self.start = self.end;
    }

    fn delete_left(&mut self, text: &mut String) {
        if self.is_empty() {
            self.end = self.start;
            self.move_left(text, true);
        }
        self.delete_selection(text);
    }

    fn delete_right(&mut self, text: &mut String) {
        if self.is_empty() {
            self.end = self.start;
            self.move_right(text, true);
        }
        self.delete_selection(text);
    }

    fn insert_char(&mut self, text: &mut String, ch: char) {
        if !self.is_empty() {
            self.delete_selection(text);
        }
        assert_eq!(self.start, self.end);
        text.insert(self.start, ch);
        self.start += ch.len_utf8();
        self.end = self.start;
    }

    fn paste(&mut self, text: &mut String, pasta: &str) {
        let normalized = self.normalized();
        if self.is_empty() {
            text.insert_str(normalized.start, pasta);
        } else {
            text.replace_range(normalized.as_range(), pasta);
        }
        self.end = normalized.start + pasta.len();
        self.start = self.end;
    }

    fn copy<'a>(&self, text: &'a str) -> Option<&'a str> {
        if self.is_empty() {
            return None;
        }
        let normalized_range = self.normalized().as_range();
        Some(&text[normalized_range])
    }
}

// TODO: consider animating scroll. maybe try to generalize Animation (from console example).
#[derive(Debug, Default, Clone, Copy)]
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

// ----

fn should_break_line(ch: char, advance_width: f32, current_x: f32, container_rect: Rect) -> bool {
    if ch == '\n' {
        return true;
    }

    assert!(current_x >= container_rect.min.x);
    let will_overflow = current_x + advance_width - container_rect.min.x > container_rect.width();
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
    container_rect: Rect,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> Range<usize> {
    let mut current_x: f32 = container_rect.min.x;
    let mut end_byte: usize = start_byte;

    for ch in (&str[start_byte..]).chars() {
        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);
        let advance_width = glyph.advance_width();

        if should_break_line(ch, advance_width, current_x, container_rect) {
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
        .get_font_instance(ctx.appearance.font_handle, ctx.appearance.font_size);

    let haiku = "With no bamboo hat\nDoes the drizzle fall on me?\nWhat care I of that?";
    tests::assert_all_glyphs_have_equal_advance_width(
        haiku,
        font_instance.reborrow_mut(),
        &mut ctx.texture_service,
    );
    // NOTE: assertion above /\ ensures that the width below \/ matches the assumption.
    let width = font_instance.typical_advance_width() * CHARS_PER_ROW as f32;
    let container_rect = Rect::new(Vec2::ZERO, Vec2::new(width, f32::INFINITY));

    let mut last_row_range = 0..0;
    while last_row_range.end < haiku.len() {
        last_row_range = layout_row(
            haiku,
            last_row_range.end,
            container_rect,
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
    container_rect: Rect,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> usize {
    let mut line_count = 0;
    let mut last_row_range = 0..0;
    while last_row_range.end < str.len() {
        last_row_range = layout_row(
            str,
            last_row_range.end,
            container_rect,
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
    scroll: TextScroll,
    coord: Vec2,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> usize {
    let left = container_rect.min.x - scroll.offset.x;
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
    scroll: TextScroll,
    coord: Vec2,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> usize {
    let top = container_rect.min.y - scroll.offset.y;
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
        scroll,
        coord,
        font_instance,
        texture_service,
    );
    last_row_range.start + offset_within_row
}

// ----
// scrolling

fn scroll_into_singleline_cursor<E: Externs>(
    str: &str,
    selection: TextSelection,
    container_rect: Rect,
    scroll: TextScroll,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> Vec2 {
    let container_width = container_rect.width();
    let cursor_width = font_instance.typical_advance_width();

    let pre_cursor_text = &str[..selection.end];
    let post_cursor_text = &str[selection.end..];

    let pre_cursor_text_width = font_instance.compute_text_width(pre_cursor_text, texture_service);
    let post_cursor_text_width =
        font_instance.compute_text_width(post_cursor_text, texture_service);
    let text_width = pre_cursor_text_width + post_cursor_text_width;

    let cursor_min_x = pre_cursor_text_width;
    let cursor_max_x = cursor_min_x + cursor_width;

    // right edge. scroll to show cursor + overscroll for cursor width.
    if cursor_max_x - scroll.offset.x > container_width {
        return scroll.offset.with_x(cursor_max_x - container_width);
    }

    // left edge. scroll to show cursor.
    if cursor_min_x < scroll.offset.x {
        return scroll.offset.with_x(cursor_min_x);
    }

    // undo overscroll when cursor moves back. if we can show all text without overscrolling, do
    // that.
    if cursor_max_x <= text_width && text_width > container_width {
        return scroll
            .offset
            .with_x(scroll.offset.x.min(text_width - container_width));
    }

    scroll.offset
}

// TODO: scroll_into_multiline_cursor need to support different row wrapping modes.
fn scroll_into_multiline_cursor<E: Externs>(
    str: &str,
    selection: TextSelection,
    container_rect: Rect,
    scroll: TextScroll,
    font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
) -> Vec2 {
    let container_height = container_rect.height();
    let font_height = font_instance.height();

    let pre_cursor_text = &str[..selection.end];
    let pre_cursor_row_count = count_rows(
        pre_cursor_text,
        container_rect,
        font_instance,
        texture_service,
    );

    let cursor_max_y = pre_cursor_row_count as f32 * font_height;
    let cursor_min_y = (cursor_max_y - font_height).max(0.0);

    // bottom edge
    if cursor_max_y - scroll.offset.y > container_height {
        return scroll.offset.with_y(cursor_max_y - container_height);
    }
    // top edge
    if cursor_min_y < scroll.offset.y {
        return scroll.offset.with_y(cursor_min_y);
    }

    scroll.offset
}

// ----
// drawing

// TODO: draw_singleline_text has way too many args. unfuck this please.
fn draw_singleline_text<E: Externs>(
    str: &str,
    container_rect: Rect,
    scroll: TextScroll,
    selection: TextSelection,
    active: bool,
    should_draw_selection: bool,
    should_draw_cursor: bool,
    appearance: &TextAppearance,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
    draw_buffer: &mut DrawBuffer<E>,
) {
    let font_ascent = font_instance.ascent();
    let font_height = font_instance.height();
    let font_typical_advance_width = font_instance.typical_advance_width();

    let mut staging_scopes = draw_buffer.multi_staging_scope();
    // NOTE: cursor_stage is techinaclly not needed because cursor can be pushed into other stage;
    // but it's nice, so why not.
    let [selection_stage, cursor_stage, glyph_stage] = staging_scopes.deref_mut();

    let normalized_selection = selection.normalized();
    let may_draw_selection = should_draw_selection && !normalized_selection.is_empty();
    let may_draw_cursor = should_draw_cursor && active;

    let mut byte_offset: usize = 0;
    let mut x_offset: f32 = container_rect.min.x - scroll.offset.x;
    let mut selection_min_x: Option<f32> = None;

    let mut draw_cursor = |min_x: f32| {
        let min = Vec2::new(min_x, container_rect.min.y);
        let size = Vec2::new(font_typical_advance_width, font_height);
        let rect = Rect::new(min, min + size);
        cursor_stage.push_rect(RectShape::new_with_fill(
            rect,
            Fill::new_with_color(appearance.cursor_bg),
        ));
    };

    // NOTE: if str is empty and we are expected to draw cursor - do it and bail out.
    if str.is_empty() {
        assert!(selection.is_empty());
        if may_draw_cursor {
            draw_cursor(x_offset);
        }
        return;
    }

    for ch in str.chars() {
        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);
        let glyph_advance_width = glyph.advance_width();

        if may_draw_selection {
            if byte_offset == normalized_selection.start {
                selection_min_x = Some(x_offset);
            }
            // NOTE: range's end is exclusive. also it is valid to start and end at the same index.
            if byte_offset + ch.len_utf8() == normalized_selection.end {
                let min_x = selection_min_x.take().expect("invalid selection range");
                let max_x = x_offset + glyph_advance_width;

                let min = Vec2::new(min_x, container_rect.min.y);
                let size = Vec2::new(max_x - min_x, font_height);
                let rect = Rect::new(min, min + size);
                selection_stage.push_rect(RectShape::new_with_fill(
                    rect,
                    Fill::new_with_color(appearance.selection_bg(active)),
                ));
            }
        }

        // TODO: consider drawing inactive cursor too, maybe outlined or something.
        if may_draw_cursor {
            if byte_offset + ch.len_utf8() == selection.end {
                draw_cursor(x_offset + glyph_advance_width);
            } else if byte_offset == 0 && selection.end == 0 {
                // ^ this works for when cursor is at the very beginning
                draw_cursor(x_offset);
            }
        }

        glyph_stage.push_rect(RectShape::new_with_fill(
            glyph
                .bounding_rect()
                .translate_by(&Vec2::new(x_offset, container_rect.min.y + font_ascent)),
            Fill::new(
                appearance.fg,
                FillTexture {
                    kind: TextureKind::Internal(glyph.tex_handle()),
                    coords: glyph.tex_coords(),
                },
            ),
        ));

        byte_offset += ch.len_utf8();
        x_offset += glyph_advance_width;
    }
}

fn draw_multiline_text<E: Externs>(
    str: &str,
    container_rect: Rect,
    scroll: TextScroll,
    selection: TextSelection,
    active: bool,
    should_draw_selection: bool,
    _should_draw_cursor: bool,
    appearance: &TextAppearance,
    mut font_instance: FontInstanceRefMut,
    texture_service: &mut TextureService<E>,
    draw_buffer: &mut DrawBuffer<E>,
) {
    let font_ascent = font_instance.ascent();
    let font_height = font_instance.height();

    let normalized_selection = selection.normalized();
    let may_draw_selection = should_draw_selection && !normalized_selection.is_empty();

    let mut staging_scopes = draw_buffer.multi_staging_scope();
    let [selection_stage, glyph_stage] = staging_scopes.deref_mut();

    let mut byte_offset: usize = 0;
    let mut xy_offset = container_rect.top_left();
    xy_offset.y -= -scroll.offset.y;
    let mut selection_min_xy: Option<Vec2> = None;
    for ch in str.chars() {
        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);
        let glyph_advance_width = glyph.advance_width();

        if may_draw_selection {
            if byte_offset == normalized_selection.start {
                selection_min_xy = Some(xy_offset);
            }
            // NOTE: range's end is exclusive. also it is valid to start and end at the same index.
            if byte_offset + ch.len_utf8() == normalized_selection.end {
                let min_xy = selection_min_xy.take().expect("fucky wacky");
                let max_xy = xy_offset + Vec2::new(glyph_advance_width, font_height);

                let rect = Rect::new(min_xy, max_xy);
                selection_stage.push_rect(RectShape::new_with_fill(
                    rect,
                    Fill::new_with_color(appearance.selection_bg(active)),
                ));
            }
        }

        if should_break_line(ch, glyph_advance_width, xy_offset.x, container_rect) {
            if let Some(min_xy) = selection_min_xy.take() {
                assert!(may_draw_selection);

                let max_xy = xy_offset + Vec2::new(glyph_advance_width, font_height);

                let rect = Rect::new(min_xy, max_xy);
                selection_stage.push_rect(RectShape::new_with_fill(
                    rect,
                    Fill::new_with_color(appearance.selection_bg(active)),
                ));

                if byte_offset + ch.len_utf8() < normalized_selection.end {
                    // the selection needs to be extended onto the next row
                    selection_min_xy =
                        Some(Vec2::new(container_rect.min.x, xy_offset.y + font_height));
                }
            }

            xy_offset.x = container_rect.min.x;
            xy_offset.y += font_height;
            if should_consume_post_line_break_char(ch) {
                byte_offset += ch.len_utf8();
                continue;
            }
        }

        glyph_stage.push_rect(RectShape::new_with_fill(
            glyph
                .bounding_rect()
                .translate_by(&Vec2::new(xy_offset.x, xy_offset.y + font_ascent)),
            Fill::new(
                appearance.fg,
                FillTexture {
                    kind: TextureKind::Internal(glyph.tex_handle()),
                    coords: glyph.tex_coords(),
                },
            ),
        ));

        byte_offset += ch.len_utf8();
        xy_offset.x += glyph_advance_width;
    }
}

// ----
// builders

pub struct TextInteractNone;
pub struct TextInteractSelect;
pub struct TextInteractEdit;

pub trait TextInteract {}

impl TextInteract for TextInteractSelect {}
impl TextInteract for TextInteractEdit {}

pub struct TextLineNone;
pub struct TextLineSingle;
pub struct TextLineMulti;

pub struct Text<Str, State, Line, Interact> {
    str: Str,
    container_rect: Rect,
    state: State,

    appearance: Option<TextAppearance>,
    key: Key,

    // NOTE: hot and active are also sort of a "state", but the kind that doesn't need to survive
    // across frames.
    hot: Option<bool>,
    active: Option<bool>,

    _interact: PhantomData<Interact>,
    _line: PhantomData<Line>,
}

type TextNonInteractive<'a> = Text<&'a str, (), TextLineNone, TextInteractNone>;
type TextNonInteractiveSingle<'a> = Text<&'a str, (), TextLineSingle, TextInteractNone>;
type TextNonInteractiveMulti<'a> = Text<&'a str, (), TextLineMulti, TextInteractNone>;

type TextSelectable<'a> = Text<&'a str, &'a mut TextState, TextLineNone, TextInteractSelect>;
type TextSelectableSingle<'a> =
    Text<&'a str, &'a mut TextState, TextLineSingle, TextInteractSelect>;
type TextSelectableMulti<'a> = Text<&'a str, &'a mut TextState, TextLineMulti, TextInteractSelect>;

type TextEditable<'a> = Text<&'a mut String, &'a mut TextState, TextLineNone, TextInteractEdit>;
type TextEditableSingle<'a> =
    Text<&'a mut String, &'a mut TextState, TextLineSingle, TextInteractEdit>;

impl<Str, State, Line, Interact> Text<Str, State, Line, Interact> {
    fn new_with_key(str: Str, container_rect: Rect, state: State, key: Key) -> Self {
        Self {
            str,
            container_rect,
            state,

            appearance: None,
            key,

            hot: None,
            active: None,

            _interact: PhantomData,
            _line: PhantomData,
        }
    }

    pub fn with_appearance(mut self, appearance: TextAppearance) -> Self {
        self.appearance = Some(appearance);
        self
    }

    fn resolved_appearance<E: Externs>(&mut self, ctx: &Context<E>) -> TextAppearance {
        self.appearance
            .take()
            .unwrap_or_else(|| TextAppearance::from_appearance(&ctx.appearance))
    }
}

impl<Str, State, Line, Interact: TextInteract> Text<Str, State, Line, Interact> {
    /// you should provide custom key when rendering stuff in a loop (maybe use
    /// [`Key::from_caller_location_and`]).
    pub fn with_key(mut self, key: Key) -> Self {
        self.key = key;
        self
    }

    /// you should provide `hot` and `active` values if you did invoke
    /// [`InteractionState::maybe_set_hot_or_active`] manually for this widget.
    pub fn with_maybe_hot_or_active(mut self, hot: bool, active: bool) -> Self {
        self.hot.replace(hot);
        self.active.replace(active);
        self
    }

    #[allow(dead_code)]
    fn is_hot(&self) -> bool {
        self.hot == Some(true)
    }

    fn is_active(&self) -> bool {
        self.active == Some(true)
    }
}

impl<'a> TextNonInteractive<'a> {
    #[track_caller]
    pub fn new_non_interactive(str: &'a str, container_rect: Rect) -> Self {
        Self::new_with_key(
            str,
            container_rect,
            (),
            Key::from_location(std::panic::Location::caller()),
        )
    }
}

impl<'a> TextSelectable<'a> {
    #[track_caller]
    pub fn new_selectable(str: &'a str, container_rect: Rect, state: &'a mut TextState) -> Self {
        Self::new_with_key(
            str,
            container_rect,
            state,
            Key::from_location(std::panic::Location::caller()),
        )
    }
}

impl<'a> TextEditable<'a> {
    #[track_caller]
    pub fn new_editable(
        str: &'a mut String,
        container_rect: Rect,
        state: &'a mut TextState,
    ) -> Self {
        Self::new_with_key(
            str,
            container_rect,
            state,
            Key::from_location(std::panic::Location::caller()),
        )
    }
}

impl<Str, State, Interact> Text<Str, State, TextLineNone, Interact> {
    pub fn singleline(self) -> Text<Str, State, TextLineSingle, Interact> {
        unsafe { (&raw const self as *const Text<Str, State, TextLineSingle, Interact>).read() }
    }

    pub fn multiline(self) -> Text<Str, State, TextLineMulti, Interact> {
        unsafe { (&raw const self as *const Text<Str, State, TextLineMulti, Interact>).read() }
    }
}

// ----
// singleline

impl<'a> TextNonInteractiveSingle<'a> {
    pub fn draw<E: Externs>(mut self, ctx: &mut Context<E>) {
        let appearance = self.resolved_appearance(ctx);
        let font_instance = ctx
            .font_service
            .get_font_instance(appearance.font_handle, appearance.font_size);
        let mut draw_buffer = ctx.draw_buffer.clip_scope(self.container_rect);

        draw_singleline_text(
            self.str,
            self.container_rect,
            TextScroll::default(),
            TextSelection::default(),
            false,
            false,
            false,
            &appearance,
            font_instance,
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );
    }
}

impl<'a> TextSelectableSingle<'a> {
    // TODO: get rid of maybe_set_hot_or_active
    fn maybe_set_hot_or_active<E: Externs>(
        &mut self,
        mut font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<E>,
        interaction_state: &mut InteractionState,
        input: &input::State,
    ) {
        let height = font_instance.height();
        let width = font_instance.compute_text_width(self.str, texture_service);
        let size = Vec2::new(width, height);
        let interaction_rect = Rect::new(self.container_rect.min, self.container_rect.min + size);

        interaction_state.maybe_set_hot_or_active(
            self.key,
            interaction_rect,
            CursorShape::Text,
            input,
        );
        self.hot.replace(interaction_state.is_hot(self.key));
        self.active.replace(interaction_state.is_active(self.key));
    }

    fn update<E: Externs>(
        &mut self,
        mut font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<E>,
        interaction_state: &mut InteractionState,
        clipboard_state: &mut ClipboardState,
        input: &input::State,
    ) {
        if self.hot.is_none() && self.active.is_none() {
            self.maybe_set_hot_or_active(
                font_instance.reborrow_mut(),
                texture_service,
                interaction_state,
                input,
            );
        }
        if !self.is_active() {
            return;
        }

        let KeyboardState { ref modifiers, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::ArrowLeft,
                    ..
                }) if modifiers.shift() => {
                    self.state.selection.move_left(self.str, true);
                }
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::ArrowRight,
                    ..
                }) if modifiers.shift() => {
                    self.state.selection.move_right(self.str, true);
                }

                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::Home,
                    ..
                }) if modifiers.shift() => {
                    self.state.selection.move_home(self.str, true);
                }
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::End,
                    ..
                }) if modifiers.shift() => {
                    self.state.selection.move_end(self.str, true);
                }

                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::C,
                    ..
                }) if modifiers.ctrl() => {
                    if let Some(copy) = self.state.selection.copy(self.str) {
                        // TODO: consider allocating copy into single-frame arena or something.
                        clipboard_state.request_write(copy.to_string());
                    }
                }

                Event::Pointer(
                    pe @ PointerEvent::Button {
                        state: ButtonState::Pressed,
                        button: Button::Primary,
                    },
                )
                | Event::Pointer(pe @ PointerEvent::Move { .. })
                    if input
                        .pointer
                        .press_origins
                        .get(&Button::Primary)
                        .is_some_and(|p| {
                            self.container_rect.contains(&Vec2::from(F64Vec2::from(*p)))
                        }) =>
                {
                    let byte_offset = locate_singleline_coord(
                        self.str,
                        self.container_rect,
                        self.state.scroll,
                        Vec2::from(F64Vec2::from(input.pointer.position)),
                        font_instance.reborrow_mut(),
                        texture_service,
                    );
                    if let PointerEvent::Button {
                        state: ButtonState::Pressed,
                        ..
                    } = pe
                    {
                        self.state.selection = TextSelection::from_range(byte_offset..byte_offset);
                    } else {
                        self.state.selection.end = byte_offset;
                    }
                }

                _ => {}
            }
        }
    }

    pub fn draw<E: Externs>(mut self, ctx: &mut Context<E>, input: &input::State) {
        let appearance = self.resolved_appearance(ctx);
        let mut font_instance = ctx
            .font_service
            .get_font_instance(appearance.font_handle, appearance.font_size);
        let mut draw_buffer = ctx.draw_buffer.clip_scope(self.container_rect);

        self.update(
            font_instance.reborrow_mut(),
            &mut ctx.texture_service,
            &mut ctx.interaction_state,
            &mut ctx.clipboard_state,
            input,
        );

        draw_singleline_text(
            self.str,
            self.container_rect,
            TextScroll::default(),
            self.state.selection,
            self.is_active(),
            true,
            false,
            &appearance,
            font_instance,
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );
    }
}

impl<'a> TextEditableSingle<'a> {
    // TODO: get rid of maybe_set_hot_or_active
    fn maybe_set_hot_or_active<E: Externs>(
        &mut self,
        mut font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<E>,
        interaction_state: &mut InteractionState,
        input: &input::State,
    ) {
        let height = font_instance.height();
        let width = if self.str.is_empty() {
            // NOTE: the problem is that if the string is empty and you are supposed to be able to
            // activate the input and start typing - with width 0 you cant.
            // this solution is not perfect, but it'll allow at least give you a tiny activation
            // area instead of none.
            //
            // TODO: how can activation (/interaction) rect of empty editable string be non-zero
            // for empty strings?
            font_instance.typical_advance_width()
        } else {
            font_instance.compute_text_width(self.str, texture_service)
        };
        let size = Vec2::new(width, height);
        let interaction_rect = Rect::new(self.container_rect.min, self.container_rect.min + size);

        interaction_state.maybe_set_hot_or_active(
            self.key,
            interaction_rect,
            CursorShape::Text,
            input,
        );
        self.hot.replace(interaction_state.is_hot(self.key));
        self.active.replace(interaction_state.is_active(self.key));
    }

    fn update<E: Externs>(
        &mut self,
        mut font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<E>,
        interaction_state: &mut InteractionState,
        clipboard_state: &mut ClipboardState,
        input: &input::State,
    ) {
        if self.hot.is_none() && self.active.is_none() {
            self.maybe_set_hot_or_active(
                font_instance.reborrow_mut(),
                texture_service,
                interaction_state,
                input,
            );
        }
        if !self.is_active() {
            return;
        }

        let KeyboardState { ref modifiers, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::ArrowLeft,
                    ..
                }) => {
                    self.state.selection.move_left(self.str, modifiers.shift());
                }
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::ArrowRight,
                    ..
                }) => {
                    self.state.selection.move_right(self.str, modifiers.shift());
                }

                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::Home,
                    ..
                }) => {
                    self.state.selection.move_home(self.str, modifiers.shift());
                }
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::End,
                    ..
                }) => {
                    self.state.selection.move_end(self.str, modifiers.shift());
                }

                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::V,
                    ..
                }) if modifiers.ctrl() => {
                    clipboard_state.request_read(self.key);
                }
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::C,
                    ..
                }) if modifiers.ctrl() => {
                    if let Some(copy) = self.state.selection.copy(self.str) {
                        // TODO: consider allocating copy into single-frame arena or something.
                        clipboard_state.request_write(copy.to_string());
                    }
                }
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::X,
                    ..
                }) if modifiers.ctrl() => {
                    if let Some(copy) = self.state.selection.copy(self.str) {
                        // TODO: consider allocating copy into single-frame arena or something.
                        clipboard_state.request_write(copy.to_string());
                        self.state.selection.delete_selection(self.str);
                    }
                }

                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::Backspace,
                    ..
                }) => {
                    self.state.selection.delete_left(self.str);
                }
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::Delete,
                    ..
                }) => {
                    self.state.selection.delete_right(self.str);
                }
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    keycode: Keycode::Char(ch),
                    ..
                }) if *ch as u32 >= 32 && *ch as u32 != 127 => {
                    // TODO: maybe better printability check ^.
                    self.state.selection.insert_char(self.str, *ch);
                }

                Event::Pointer(
                    ev @ PointerEvent::Button {
                        state: ButtonState::Pressed,
                        button: Button::Primary,
                    },
                )
                | Event::Pointer(ev @ PointerEvent::Move { .. })
                    if input
                        .pointer
                        .press_origins
                        .get(&Button::Primary)
                        .is_some_and(|p| {
                            self.container_rect.contains(&Vec2::from(F64Vec2::from(*p)))
                        }) =>
                {
                    let byte_offset = locate_singleline_coord(
                        self.str,
                        self.container_rect,
                        self.state.scroll,
                        Vec2::from(F64Vec2::from(input.pointer.position)),
                        font_instance.reborrow_mut(),
                        texture_service,
                    );
                    if let PointerEvent::Button {
                        state: ButtonState::Pressed,
                        ..
                    } = ev
                    {
                        self.state.selection = TextSelection::from_range(byte_offset..byte_offset);
                    } else {
                        self.state.selection.end = byte_offset;
                    }
                }

                _ => {}
            }
        }

        if let Some(pasta) = clipboard_state.try_take_read(self.key) {
            // TODO: consider removing line breaks or something.
            self.state.selection.paste(self.str, pasta.as_str());
        }

        // NOTE: cursor must be updated in reaction to possible interactions (above ^).
        self.state.scroll.offset = scroll_into_singleline_cursor(
            self.str,
            self.state.selection,
            self.container_rect,
            self.state.scroll,
            font_instance,
            texture_service,
        );
    }

    pub fn draw<E: Externs>(mut self, ctx: &mut Context<E>, input: &input::State) {
        let appearance = self.resolved_appearance(ctx);
        let mut font_instance = ctx
            .font_service
            .get_font_instance(appearance.font_handle, appearance.font_size);
        let mut draw_buffer = ctx.draw_buffer.clip_scope(self.container_rect);

        self.update(
            font_instance.reborrow_mut(),
            &mut ctx.texture_service,
            &mut ctx.interaction_state,
            &mut ctx.clipboard_state,
            input,
        );

        draw_singleline_text(
            self.str,
            self.container_rect,
            self.state.scroll,
            self.state.selection,
            self.is_active(),
            true,
            true,
            &appearance,
            font_instance,
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );
    }
}

// ----
// multiline

impl<'a> TextNonInteractiveMulti<'a> {
    pub fn draw<E: Externs>(mut self, ctx: &mut Context<E>) {
        let appearance = self.resolved_appearance(ctx);
        let font_instance = ctx
            .font_service
            .get_font_instance(appearance.font_handle, appearance.font_size);
        let mut draw_buffer = ctx.draw_buffer.clip_scope(self.container_rect);

        draw_multiline_text(
            self.str,
            self.container_rect,
            TextScroll::default(),
            TextSelection::default(),
            false,
            false,
            false,
            &appearance,
            font_instance,
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );
    }
}

impl<'a> TextSelectableMulti<'a> {
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
            self.str,
            self.container_rect,
            font_instance,
            texture_service,
        );
        let height = row_count as f32 * font_height;
        let size = Vec2::new(self.container_rect.width(), height);

        let interaction_rect = Rect::new(self.container_rect.min, self.container_rect.min + size);
        interaction_state.maybe_set_hot_or_active(
            self.key,
            interaction_rect,
            CursorShape::Text,
            input,
        );
        self.hot.replace(interaction_state.is_hot(self.key));
        self.active.replace(interaction_state.is_active(self.key));
    }

    fn update<E: Externs>(
        &mut self,
        mut font_instance: FontInstanceRefMut,
        texture_service: &mut TextureService<E>,
        interaction_state: &mut InteractionState,
        clipboard_state: &mut ClipboardState,
        input: &input::State,
    ) {
        if self.hot.is_none() && self.active.is_none() {
            self.maybe_set_hot_or_active(
                font_instance.reborrow_mut(),
                texture_service,
                interaction_state,
                input,
            );
        }
        if !self.is_active() {
            return;
        }

        let KeyboardState { ref modifiers, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::ArrowLeft,
                    ..
                }) if modifiers.shift() => {
                    self.state.selection.move_left(self.str, true);
                }
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::ArrowRight,
                    ..
                }) if modifiers.shift() => {
                    self.state.selection.move_right(self.str, true);
                }

                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::Home,
                    ..
                }) if modifiers.shift() => {
                    self.state.selection.move_home(self.str, true);
                }
                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::End,
                    ..
                }) if modifiers.shift() => {
                    self.state.selection.move_end(self.str, true);
                }

                Event::Keyboard(KeyboardEvent::Key {
                    state: KeyState::Pressed,
                    scancode: Scancode::C,
                    ..
                }) if modifiers.ctrl() => {
                    if let Some(copy) = self.state.selection.copy(self.str) {
                        // TODO: consider allocating copy into single-frame arena or something.
                        clipboard_state.request_write(copy.to_string());
                    }
                }

                Event::Pointer(
                    pe @ PointerEvent::Button {
                        state: ButtonState::Pressed,
                        button: Button::Primary,
                    },
                )
                | Event::Pointer(pe @ PointerEvent::Move { .. })
                    if input
                        .pointer
                        .press_origins
                        .get(&Button::Primary)
                        .is_some_and(|p| {
                            self.container_rect.contains(&Vec2::from(F64Vec2::from(*p)))
                        }) =>
                {
                    let byte_offset = locate_multiline_coord(
                        self.str,
                        self.container_rect,
                        self.state.scroll,
                        Vec2::from(F64Vec2::from(input.pointer.position)),
                        font_instance.reborrow_mut(),
                        texture_service,
                    );
                    if let PointerEvent::Button {
                        state: ButtonState::Pressed,
                        ..
                    } = pe
                    {
                        self.state.selection = TextSelection::from_range(byte_offset..byte_offset);
                    } else {
                        self.state.selection.end = byte_offset;
                    }
                }

                _ => {}
            }
        }

        // NOTE: cursor must be updated in reaction to possible interactions (above ^).
        self.state.scroll.offset = scroll_into_multiline_cursor(
            self.str,
            self.state.selection,
            self.container_rect,
            self.state.scroll,
            font_instance,
            texture_service,
        );
    }

    pub fn draw<E: Externs>(mut self, ctx: &mut Context<E>, input: &input::State) {
        let appearance = self.resolved_appearance(ctx);
        let mut font_instance = ctx
            .font_service
            .get_font_instance(appearance.font_handle, appearance.font_size);
        let mut draw_buffer = ctx.draw_buffer.clip_scope(self.container_rect);

        self.update(
            font_instance.reborrow_mut(),
            &mut ctx.texture_service,
            &mut ctx.interaction_state,
            &mut ctx.clipboard_state,
            input,
        );

        draw_multiline_text(
            self.str,
            self.container_rect,
            self.state.scroll,
            self.state.selection,
            self.is_active(),
            true,
            false,
            &appearance,
            font_instance,
            &mut ctx.texture_service,
            draw_buffer.deref_mut(),
        );
    }
}
