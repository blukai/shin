use std::ops::Range;

use crate::{Rect, Rgba8, TextureHandle, Vec2};

#[derive(Debug, Clone, PartialEq)]
pub struct TextureFill {
    pub handle: TextureHandle,
    pub coords: Rect,
    pub base_color: Rgba8,
}

impl TextureFill {
    pub fn new(handle: TextureHandle) -> Self {
        Self {
            handle,
            coords: Rect::new(Vec2::splat(0.0), Vec2::splat(1.0)),
            base_color: Rgba8::WHITE,
        }
    }

    pub fn with_coords(mut self, coords: Rect) -> Self {
        self.coords = coords;
        self
    }

    pub fn with_base_color(mut self, base_color: Rgba8) -> Self {
        self.base_color = base_color;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Fill {
    Color(Rgba8),
    Texture(TextureFill),
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(i32)]
pub enum StrokeAlignment {
    Inside = -1,
    Outside = 1,
    // TODO: consider changing default stroke alignment from center to outside.
    // NOTE: center alignment doesn't work well with 1px width.
    #[default]
    Center = 0,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub color: Rgba8,
    pub alignment: StrokeAlignment,
}

impl Stroke {
    // NOTE: alignment is omitted because in major majority of cases it's center (the default).
    pub fn new(width: f32, color: Rgba8) -> Self {
        Self {
            width,
            color,
            alignment: StrokeAlignment::default(),
        }
    }

    pub fn with_alignment(mut self, alignment: StrokeAlignment) -> Self {
        self.alignment = alignment;
        self
    }
}

#[derive(Debug)]
pub struct RectShape {
    pub rect: Rect,
    pub fill: Option<Fill>,
    // TODO: i want to be able to specify rect stroke per-side.
    //   top and left, or maybe only bottom, etc.
    //   would that still be called "stroke"?
    pub stroke: Option<Stroke>,
    // TODO: rect shape must support 4 distinct corner radii.
    pub corner_radius: Option<f32>,
}

impl RectShape {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            fill: None,
            stroke: None,
            corner_radius: None,
        }
    }

    pub fn with_fill(mut self, fill: Option<Fill>) -> Self {
        self.fill = fill;
        self
    }

    pub fn with_stroke(mut self, stroke: Option<Stroke>) -> Self {
        self.stroke = stroke;
        self
    }

    pub fn with_corner_radius(mut self, corner_radius: Option<f32>) -> Self {
        self.corner_radius = corner_radius;
        self
    }
}

#[derive(Debug)]
pub struct LineShape {
    pub points: [Vec2; 2],
    // TODO: line shape must not depend on stroke thing.
    //   pull out width and color. drop alignment.
    pub stroke: Stroke,
}

impl LineShape {
    pub fn new(a: Vec2, b: Vec2, stroke: Stroke) -> Self {
        Self {
            points: [a, b],
            stroke,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RectSdfParams {
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub corner_radius: f32,
    pub stroke_width: f32,
    pub stroke_color: [f32; 4],
    pub stroke_alignment: i32, // -1 inside, 0 center, 1 outside
}

#[derive(Debug, Clone, PartialEq)]
pub enum SdfParams {
    Rect(RectSdfParams),
}

#[repr(C)]
#[derive(Debug)]
pub struct Vertex {
    /// screen pixel coordinates.
    /// 0, 0 is the top left corner of the screen.
    pub pos: Vec2,
    pub color: Rgba8,
    /// normalized texture coordinates.
    /// 0, 0 is the top left corner of the texture.
    /// 1, 1 is the bottom right corner of the texture.
    pub tex_coord: Vec2,
}

#[derive(Debug)]
pub struct DrawCommand {
    pub index_range: Range<u32>,
    pub scissor: Option<Rect>,
    pub texture: Option<TextureHandle>,
    pub sdf_params: Option<SdfParams>,
}

#[derive(Debug, Default)]
pub struct DrawData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub commands: Vec<DrawCommand>,
    pending_indices: u32,
    current_scissor_rect: Option<Rect>,
    current_texture: Option<TextureHandle>,
    current_sdf_params: Option<SdfParams>,
}

impl DrawData {
    fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();
        assert_eq!(self.pending_indices, 0);
        self.current_scissor_rect = None;
        self.current_texture = None;
        self.current_sdf_params = None;
    }

    fn flush(&mut self) {
        if self.pending_indices == 0 {
            return;
        }

        let end_index = self.indices.len() as u32;
        let start_index = end_index - self.pending_indices;
        self.commands.push(DrawCommand {
            index_range: start_index..end_index,
            scissor: self.current_scissor_rect,
            texture: self.current_texture,
            sdf_params: self.current_sdf_params.clone(),
        });

        self.pending_indices = 0;
    }

    fn set_scissor_rect(&mut self, rect: Option<Rect>) {
        let prev = self.current_scissor_rect;
        let next = match (prev, rect) {
            (Some(prev), Some(next)) => Some(next.clamp(prev)),
            (_, next) => next,
        };
        if prev == next {
            return;
        }
        self.flush();
        self.current_scissor_rect = next;
    }

    fn set_texture(&mut self, texture: Option<TextureHandle>) {
        if self.current_texture == texture {
            return;
        }
        self.flush();
        self.current_texture = texture;
    }

    fn set_sdf_params(&mut self, sdf_params: Option<SdfParams>) {
        if self.current_sdf_params == sdf_params {
            return;
        }
        self.flush();
        self.current_sdf_params = sdf_params;
    }

    fn push_vertex(&mut self, pos: Vec2, color: Rgba8, tex_coord: Vec2) {
        self.vertices.push(Vertex {
            pos,
            color,
            tex_coord,
        });
    }

    fn push_indices(&mut self, i0: u32, i1: u32, i2: u32) {
        self.indices.push(i0);
        self.indices.push(i1);
        self.indices.push(i2);
        self.pending_indices += 3;
    }

    fn push_quad(&mut self, rect: Rect, color: Rgba8, tex_coords: Rect) {
        let i = self.vertices.len() as u32;

        self.push_vertex(rect.top_left(), color, tex_coords.top_left());
        self.push_vertex(rect.top_right(), color, tex_coords.top_right());
        self.push_vertex(rect.bottom_right(), color, tex_coords.bottom_right());
        self.push_vertex(rect.bottom_left(), color, tex_coords.bottom_left());

        // top left -> top right -> bottom right
        self.push_indices(i + 0, i + 1, i + 2);
        // bottom right -> bottom left -> top left
        self.push_indices(i + 2, i + 3, i + 0);
    }
}

// NOTE: this will go away (i think).
#[derive(Debug, Default)]
pub struct DrawBuffer {
    draw_data: DrawData,
}

impl DrawBuffer {
    pub fn flush(&mut self) {
        self.draw_data.flush();
    }

    pub fn draw_data(&self) -> &DrawData {
        &self.draw_data
    }

    pub fn clear(&mut self) {
        self.draw_data.clear();
    }

    // NOTE: you don't this to have methods that return ScopeGuard.
    // that shit turned out to be very annoying for various reasons (related to rusts borrowing
    // rules).
    // if you want scope guard thing - you should implement it at application-level.

    pub fn scissor_rect(&self) -> Option<Rect> {
        self.draw_data.current_scissor_rect
    }

    pub fn set_scissor_rect(&mut self, rect: Option<Rect>) {
        self.draw_data.set_scissor_rect(rect)
    }

    fn push_rect_filled(&mut self, rect: Rect, fill: Fill) {
        let (color, tex_coords, tex_handle) = match fill {
            Fill::Color(color) => (color, Rect::new(Vec2::splat(0.0), Vec2::splat(1.0)), None),
            Fill::Texture(TextureFill {
                handle,
                coords,
                base_color,
            }) => (base_color, coords, Some(handle)),
        };

        self.draw_data.set_texture(tex_handle);
        self.draw_data.push_quad(rect, color, tex_coords);
    }

    pub fn push_line(&mut self, line_shape: LineShape) {
        // NOTE: line's stroke may only be centered.
        //   specifying outside/inside stroke alignment makes sense only for shapes that need an
        //   outline (/ need to be stroked).
        assert!(matches!(
            line_shape.stroke.alignment,
            StrokeAlignment::Center
        ));

        // NOTE: there's no sdf params for line.
        self.draw_data.set_sdf_params(None);

        // computes the vertex position offset away the from center caused by line width.
        #[inline]
        fn compute_line_width_offset(a: Vec2, b: Vec2, width: f32) -> Vec2 {
            // direction defines how the line is oriented in space. it allows to know
            // which way to move the vertices to create the desired width.
            let dir = b - a;

            // normalizing the direction vector converts it into a unit vector (length
            // of 1). normalization ensures that the offset is proportional to the line
            // width, regardless of the line's length.
            let norm_dir = dir.normalize_or_zero();

            // create a vector that points outward from the line. we want to move the
            // vertices away from the center of the line, not along its length.
            let perp = norm_dir.perp();

            // to distribute the offset evenly on both sides of the line
            let offset = perp * (width * 0.5);

            offset
        }

        let [a, b] = line_shape.points;
        let Stroke { width, color, .. } = line_shape.stroke;
        let offset = compute_line_width_offset(a, b, width);
        self.push_rect_filled(Rect::new(a + offset, b - offset), Fill::Color(color));
    }

    pub fn push_rect(&mut self, rect_shape: RectShape) {
        let RectShape {
            mut rect,
            fill,
            stroke,
            corner_radius,
        } = rect_shape;
        match (stroke, corner_radius) {
            (None, None) => {
                self.draw_data.set_sdf_params(None);
            }
            (stroke, corner_radius) => {
                let mut rect_sdf = RectSdfParams {
                    center: rect.center().to_array(),
                    size: rect.size().to_array(),
                    corner_radius: corner_radius.unwrap_or(0.0),
                    stroke_width: 0.0,
                    stroke_color: Rgba8::TRANSPARENT.to_f32_array(),
                    stroke_alignment: StrokeAlignment::default() as i32,
                };
                if let Some(ref stroke) = stroke {
                    rect_sdf.stroke_width = stroke.width;
                    rect_sdf.stroke_color = stroke.color.to_f32_array();
                    rect_sdf.stroke_alignment = stroke.alignment as i32;
                    match stroke.alignment {
                        StrokeAlignment::Inside => {}
                        // NOTE: in cases of outside/center outline rect needs to be scaled up.
                        // this does not change size of the rect, no; but "reserves" space for the
                        // outline.
                        StrokeAlignment::Outside => {
                            rect = rect.inflate(Vec2::splat(stroke.width));
                        }
                        StrokeAlignment::Center => {
                            rect = rect.inflate(Vec2::splat(stroke.width * 0.5));
                        }
                    }
                }
                self.draw_data
                    .set_sdf_params(Some(SdfParams::Rect(rect_sdf)));
            }
        };

        self.push_rect_filled(rect, fill.unwrap_or(Fill::Color(Rgba8::TRANSPARENT)));
    }
}
