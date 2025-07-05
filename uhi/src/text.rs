use std::ops::Range;

use glam::Vec2;
use input::Scancode;

use crate::{Context, Externs, Fill, FillTexture, FontHandle, Rect, RectShape, Rgba8, TextureKind};

// TODO: key repeat (input)
// TODO: multiline
// TODO: per-char layout styling
// TODO: filters / input types (number-only, etc.)

// TODO: color schemes ?
const FG: Rgba8 = Rgba8::WHITE;
const CURSOR_ACTIVE: Rgba8 = Rgba8::from_u32(0x8faf9fff);
const CURSOR_INACTIVE: Rgba8 = Rgba8::from_u32(0x8faf9fff);
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
    readonly: bool,
    // if equal, no selection; start may be less than or greater than end (start is where the
    // initial click was).
    cursor: Range<usize>,
}

impl Default for TextState {
    // TODO: add id into TextState and track focus within the Context (by id).
    //
    // generate id with #[track_caller] + Location thing; allow to specify id manually (because
    // when rendering lists location-based id would dup in a loop).
    #[track_caller]
    fn default() -> Self {
        Self {
            readonly: Default::default(),
            cursor: Default::default(),
        }
    }
}

impl TextState {
    pub fn readonly(mut self, value: bool) -> Self {
        self.readonly = value;
        self
    }

    // ----

    fn has_selection(&self) -> bool {
        self.cursor.start != self.cursor.end
    }

    // TODO: move modifiers (by char, by char type, by word, etc.)

    fn move_cursor_left(&mut self, _text: &str, extend_selection: bool) {
        if self.has_selection() && !extend_selection {
            self.cursor.end = self.cursor.end.min(self.cursor.start);
            self.cursor.start = self.cursor.end;
            return;
        }

        self.cursor.end = self.cursor.end.saturating_sub(1);
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

        self.cursor.end = (self.cursor.end + 1).min(text.len());
        if !extend_selection {
            self.cursor.start = self.cursor.end;
        }
    }

    // TODO: mouse selection
}

// ----

// TODO: hover, focus / activation and stuff
pub fn update_text(
    text: &mut String,
    state: &mut TextState,
    appearance: &TextAppearance,
    input: &input::State,
) {
    use Scancode::*;
    let scancodes = &input.keyboard.scancodes;
    if scancodes.just_pressed(ArrowLeft) {
        state.move_cursor_left(text, scancodes.any_pressed([ShiftLeft, ShiftRight]));
    }
    if scancodes.just_pressed(ArrowRight) {
        state.move_cursor_right(text, scancodes.any_pressed([ShiftLeft, ShiftRight]));
    }
}

pub fn draw_text<E: Externs>(
    text: &str,
    // NOTE: if state is None - text is not interactable.
    state: Option<&TextState>,
    appearance: &TextAppearance,
    // TODO: consider replacing position with Placement enum or something?
    // - singleline variant will need an position and width.
    // - multiline variant will need an area rect.
    position: Vec2,
    ctx: &mut Context<E>,
) {
    let mut font_instance_ref = ctx
        .font_service
        .get_font_instance_mut(appearance.font_handle, appearance.font_size);

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
                // TODO: text edit activation/deactivation
                Fill::with_color(SELECTION_ACTIVE),
            ));
        }

        if !state.readonly {
            let cursor_rect = {
                const CURSOR_WIDTH: f32 = 2.0;
                let mut min = position + Vec2::new(cursor_end_x, 0.0);
                if state.cursor.end == 0 {
                    min -= CURSOR_WIDTH;
                }
                let size = Vec2::new(CURSOR_WIDTH, font_instance_ref.line_height());
                Rect::new(min, min + size)
            };

            ctx.draw_buffer.push_rect(RectShape::with_fill(
                cursor_rect,
                // TODO: text edit activation/deactivation
                Fill::with_color(CURSOR_ACTIVE),
            ));
        }
    }

    let fg = appearance.fg.unwrap_or(Rgba8::WHITE);
    let mut offset_x = position.x;
    let ascent = font_instance_ref.ascent();
    for ch in text.chars() {
        let char_ref = font_instance_ref.get_char(ch, &mut ctx.texture_service);
        let char_bounds = char_ref.bounds();

        ctx.draw_buffer.push_rect(RectShape::with_fill(
            char_bounds.translate_by(&Vec2::new(offset_x, position.y + ascent)),
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
