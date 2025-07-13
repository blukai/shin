use std::ops::Range;

use input::{Event, KeyboardEvent, KeyboardState, PointerButton, PointerEvent, Scancode};

use crate::{
    Context, Externs, F64Vec2, Fill, FillTexture, FontHandle, FontInstanceRefMut, Key, Rect,
    RectShape, Rgba8, TextureKind, TextureService, Vec2,
};

// TODO: vertically and horizontally scrollable editors
//
// TODO: per-char layout styling
// - should be able to make some fragments of text bold?
// - should be able to change some elements of appearance (fg, bg)
//
// TODO: filters / input types
// - for example number-only input, etc.
//
// TODO: color schemes ?

const FG: Rgba8 = Rgba8::WHITE;
const SELECTION_ACTIVE: Rgba8 = Rgba8::from_u32(0x304a3dff);
const SELECTION_INACTIVE: Rgba8 = Rgba8::from_u32(0x484848ff);
const CURSOR: Rgba8 = Rgba8::from_u32(0x8faf9fff);

#[derive(Default)]
pub struct TextSelection {
    // if equal, no selection; start may be less than or greater than end (start is where the
    // initial click was).
    cursor: Range<usize>,
}

impl TextSelection {
    fn is_empty(&self) -> bool {
        self.cursor.start == self.cursor.end
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

    // TODO: mouse selection (sometking like drag or drag_start and drag end?)
}

pub struct Text<'a> {
    text: &'a str,
    font_handle: FontHandle,
    font_size: f32,
    rect: Rect,

    pub fg: Rgba8,
}

impl<'a> Text<'a> {
    pub fn new(text: &'a str, font_handle: FontHandle, font_size: f32, rect: Rect) -> Self {
        Self {
            text,
            font_handle,
            font_size,
            rect,

            fg: FG,
        }
    }

    pub fn with_fg(mut self, value: Rgba8) -> Self {
        self.fg = value;
        self
    }

    pub fn singleline(self) -> TextSingleline<'a> {
        TextSingleline::new(self)
    }
}

pub struct TextSingleline<'a> {
    base: Text<'a>,
}

impl<'a> TextSingleline<'a> {
    pub fn new(base: Text<'a>) -> Self {
        Self { base }
    }

    pub fn draw<E: Externs>(self, ctx: &mut Context<E>) {
        let Text {
            text,
            font_handle,
            font_size,
            rect,
            fg,
        } = self.base;

        let mut font_instance_ref = ctx
            .font_service
            .get_font_instance_mut(font_handle, font_size);
        let ascent = font_instance_ref.ascent();

        let mut offset_x: f32 = rect.min.x;
        for ch in text.chars() {
            let char_ref = font_instance_ref.get_char(ch, &mut ctx.texture_service);

            ctx.draw_buffer.push_rect(RectShape::with_fill(
                char_ref
                    .bounding_rect()
                    .translate_by(&Vec2::new(offset_x, rect.min.y + ascent)),
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

    pub fn selectable(self, selection: &'a mut TextSelection) -> TextSinglelineSelectable<'a> {
        TextSinglelineSelectable::new(self, selection)
    }
}

pub struct TextSinglelineSelectable<'a> {
    singleline: TextSingleline<'a>,
    selection: &'a mut TextSelection,

    pub selection_active: Rgba8,
    pub selection_inactive: Rgba8,
    pub hot: bool,
    pub active: bool,
}

impl<'a> TextSinglelineSelectable<'a> {
    pub fn new(singleline: TextSingleline<'a>, selection: &'a mut TextSelection) -> Self {
        Self {
            singleline,
            selection,
            selection_active: SELECTION_ACTIVE,
            selection_inactive: SELECTION_INACTIVE,
            hot: false,
            active: false,
        }
    }

    pub fn with_selection_active(mut self, value: Rgba8) -> Self {
        self.selection_active = value;
        self
    }

    pub fn with_selection_inactive(mut self, value: Rgba8) -> Self {
        self.selection_inactive = value;
        self
    }

    pub fn with_hot(mut self, value: bool) -> Self {
        self.hot = value;
        self
    }

    pub fn with_active(mut self, value: bool) -> Self {
        self.active = value;
        self
    }

    pub fn compute_size<E: Externs>(&self, ctx: &mut Context<E>) -> Vec2 {
        let Text {
            text,
            font_handle,
            font_size,
            ..
        } = self.singleline.base;
        let mut font_instance_ref = ctx
            .font_service
            .get_font_instance_mut(font_handle, font_size);
        let width = font_instance_ref.compute_text_width(text, &mut ctx.texture_service);
        let height = font_instance_ref.height();
        Vec2::new(width, height)
    }

    // returns(if some) byte offset(not char index)
    fn locate_char<E: Externs>(&self, position: Vec2, ctx: &mut Context<E>) -> Option<usize> {
        let Text {
            text,
            font_handle,
            font_size,
            ref rect,
            ..
        } = self.singleline.base;

        let mut font_instance_ref = ctx
            .font_service
            .get_font_instance_mut(font_handle, font_size);

        let mut byte_offset: usize = 0;
        let mut offset_x: f32 = rect.min.x;

        for ch in text.chars() {
            let char_ref = font_instance_ref.get_char(ch, &mut ctx.texture_service);

            let start_x = offset_x;
            let end_x = start_x + char_ref.advance_width();
            if position.x >= start_x && position.x <= end_x {
                // NOTE: it seems like everyone consider char selected only if you're reaching past
                // half of it.
                let mid_x = start_x + char_ref.advance_width() / 2.0;
                if position.x < mid_x {
                    return Some(byte_offset);
                } else {
                    return Some(byte_offset + ch.len_utf8());
                }
            }

            byte_offset += ch.len_utf8();
            offset_x += char_ref.advance_width();
        }

        None
    }

    pub fn update<E: Externs>(self, ctx: &mut Context<E>, input: &input::State) -> Self {
        let Text { text, .. } = self.singleline.base;
        let KeyboardState { ref scancodes, .. } = input.keyboard;
        for event in input.events.iter() {
            match event {
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowLeft,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    self.selection.move_cursor_left(text, true);
                }
                Event::Keyboard(KeyboardEvent::Press {
                    scancode: Scancode::ArrowRight,
                    ..
                }) if scancodes.any_pressed([Scancode::ShiftLeft, Scancode::ShiftRight]) => {
                    self.selection.move_cursor_right(text, true);
                }

                Event::Pointer(PointerEvent::Press {
                    button: PointerButton::Primary,
                }) if self.hot /* NOTE: we want to react only to "inside" presses. */ => {
                    let position = Vec2::from(F64Vec2::from(input.pointer.position));
                    if let Some(byte_offset) = self.locate_char(position, ctx) {
                        self.selection.cursor = byte_offset..byte_offset;
                    }
                }
                Event::Pointer(PointerEvent::Motion { .. })
                    if input.pointer.buttons.pressed(PointerButton::Primary) =>
                {
                    let position = Vec2::from(F64Vec2::from(input.pointer.position));
                    if let Some(byte_offset) = self.locate_char(position, ctx) {
                        self.selection.cursor.end = byte_offset;
                    }
                }
                _ => {}
            }
        }

        self
    }

    pub fn maybe_set_hot_or_active<E: Externs>(
        mut self,
        key: Key,
        ctx: &mut Context<E>,
        input: &input::State,
    ) -> Self {
        let Text { ref rect, .. } = self.singleline.base;
        let interaction_rect = Rect::new(rect.min, rect.min + self.compute_size(ctx));
        ctx.interaction_state
            .maybe_set_hot_or_active(key, interaction_rect, input);
        self.hot = ctx.interaction_state.is_hot(key);
        self.active = ctx.interaction_state.is_active(key);
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
        let Text {
            text,
            font_handle,
            font_size,
            ref rect,
            ..
        } = self.singleline.base;

        let mut font_instance_ref = ctx
            .font_service
            .get_font_instance_mut(font_handle, font_size);

        let cursor_end_x = font_instance_ref
            .compute_text_width(&text[..self.selection.cursor.end], &mut ctx.texture_service);

        if !self.selection.is_empty() {
            let cursor_start_x = font_instance_ref.compute_text_width(
                &text[..self.selection.cursor.start],
                &mut ctx.texture_service,
            );
            let left = cursor_start_x.min(cursor_end_x);
            let right = cursor_start_x.max(cursor_end_x);
            let selection_rect = {
                // TODO: multiline text support
                let min = rect.min + Vec2::new(left, 0.0);
                let size = Vec2::new(right - left, font_instance_ref.height());
                Rect::new(min, min + size)
            };

            ctx.draw_buffer.push_rect(RectShape::with_fill(
                selection_rect,
                Fill::with_color(if self.active {
                    SELECTION_ACTIVE
                } else {
                    SELECTION_INACTIVE
                }),
            ));
        }

        self.singleline.draw(ctx);
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
