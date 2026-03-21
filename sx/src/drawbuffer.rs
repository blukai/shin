use std::ops::Range;
use std::ptr::NonNull;
use std::slice;
use std::vec::Drain;

use crate::{Rect, Rgba8, TextureHandle, Vec2};

// TODO: i am not quite happy with the word "brush" here, but i got no better ideas atm; it's
// better and more correct then "fill" that i used previously.

#[derive(Debug, Clone, PartialEq)]
pub struct TextureBrush {
    pub handle: TextureHandle,
    pub coords: Rect,
    pub base_color: Rgba8,
}

impl TextureBrush {
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

// TODO: shader brush

#[derive(Debug, Clone, PartialEq)]
pub enum Brush {
    Solid(Rgba8),
    Texture(TextureBrush),
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum StrokeAlignment {
    Inside,
    Outside,
    // TODO: consider changing default stroke alignment from center to outside.
    // NOTE: center alignment wouldn't work well with 1px width.
    #[default]
    Center,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub alignment: StrokeAlignment,
    pub brush: Brush,
}

impl Stroke {
    // NOTE: alignment is omitted because in major majority of cases it's center (the default).
    pub fn new(width: f32, brush: Brush) -> Self {
        Self {
            width,
            alignment: StrokeAlignment::default(),
            brush,
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
    pub brush: Option<Brush>,
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
            brush: None,
            stroke: None,
            corner_radius: None,
        }
    }

    pub fn with_brush(mut self, brush: Option<Brush>) -> Self {
        self.brush = brush;
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

// TODO: get rid of RectSdf.
//   you want composable shader brushes or something.
#[derive(Debug, Clone, PartialEq)]
pub struct RectSdf {
    pub center: Vec2,
    pub size: Vec2,
    pub corner_radius: Option<f32>,
    pub stroke: Option<Stroke>,
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
pub enum SdfParams {
    Rect(RectSdf),
}

// TODO: instancing (to enable batching (vertices will be able to exist in 0..1 coordinate space
// (probably) and then they can be translated, scaled, rotated with instance transforms (for
// example this will allow to render all rects within a single draw call? or am i being
// delusional?))).

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
    pub texture: Option<TextureHandle>,
    pub scissor: Option<Rect>,
    pub sdf_params: Option<SdfParams>,
}

#[derive(Debug, Default)]
pub struct DrawLayerData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub commands: Vec<DrawCommand>,
    pending_indices: u32,
    current_texture: Option<TextureHandle>,
    current_scissor_rect: Option<Rect>,
    current_sdf_params: Option<SdfParams>,
}

impl DrawLayerData {
    fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();
        assert_eq!(self.pending_indices, 0);
        self.current_texture = None;
        self.current_scissor_rect = None;
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
            texture: self.current_texture,
            scissor: self.current_scissor_rect,
            sdf_params: self.current_sdf_params.clone(),
        });

        self.pending_indices = 0;
    }

    fn set_texture(&mut self, texture: Option<TextureHandle>) {
        if self.current_texture == texture {
            return;
        }
        self.flush();
        self.current_texture = texture;
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

    fn push_triangle(&mut self, zero: u32, ichi: u32, ni: u32) {
        self.indices.push(zero);
        self.indices.push(ichi);
        self.indices.push(ni);
        self.pending_indices += 3;
    }
}

pub struct DrawLayerFlush<'a> {
    pub vertices: &'a [Vertex],
    pub indices: &'a [u32],
    // drain so that you can take ownership of values
    pub commands: Drain<'a, DrawCommand>,
}

pub struct DrawLayersDrain<'a> {
    iter: slice::IterMut<'a, DrawLayerData>,
    ptr: NonNull<DrawBuffer>,
}

impl<'a> Iterator for DrawLayersDrain<'a> {
    type Item = DrawLayerFlush<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|layer| {
            layer.flush();
            DrawLayerFlush {
                vertices: layer.vertices.as_slice(),
                indices: layer.indices.as_slice(),
                commands: layer.commands.drain(..),
            }
        })
    }
}

impl<'a> Drop for DrawLayersDrain<'a> {
    fn drop(&mut self) {
        // SAFETY: we have exclusive access to draw buffer
        unsafe { self.ptr.as_mut() }.clear();
    }
}

// NOTE: the initial idea for why i did implement this didn't work out, but it doesn't mean that
// the implementation is completely useless. this will probably work pretty well for tooptips and
// stuff.
#[repr(usize)]
#[derive(Debug, Default, Clone, Copy)]
pub enum DrawLayer {
    #[default]
    Main,
}

impl DrawLayer {
    pub const MAX: usize = 1;
}

#[derive(Debug, Default)]
pub struct DrawBuffer {
    layer: DrawLayer,
    layers: [DrawLayerData; DrawLayer::MAX],
}

impl DrawBuffer {
    #[inline(always)]
    fn draw_data(&self) -> &DrawLayerData {
        &self.layers[self.layer as usize]
    }

    #[inline(always)]
    fn draw_data_mut(&mut self) -> &mut DrawLayerData {
        &mut self.layers[self.layer as usize]
    }

    // NOTE: you don't this to have methods that return ScopeGuard.
    // that shit turned out to be very annoying for various reasons (related to rusts borrowing
    // rules).
    // if you want scope guard thing - you should implement it at application-level.

    pub fn layer(&mut self) -> DrawLayer {
        self.layer
    }

    pub fn set_layer(&mut self, layer: DrawLayer) {
        self.layer = layer;
    }

    pub fn scissor_rect(&self) -> Option<Rect> {
        self.draw_data().current_scissor_rect
    }

    pub fn set_scissor_rect(&mut self, rect: Option<Rect>) {
        self.draw_data_mut().set_scissor_rect(rect)
    }

    fn push_rect_filled(&mut self, rect: Rect, brush: Brush) {
        let (color, tex_coords, tex_handle) = match brush {
            Brush::Solid(color) => (color, Rect::new(Vec2::splat(0.0), Vec2::splat(1.0)), None),
            Brush::Texture(TextureBrush {
                handle,
                coords,
                base_color,
            }) => (base_color, coords, Some(handle)),
        };

        let draw_data = self.draw_data_mut();
        draw_data.set_texture(tex_handle);
        let idx = draw_data.vertices.len() as u32;

        draw_data.push_vertex(rect.top_left(), color, tex_coords.top_left());
        draw_data.push_vertex(rect.top_right(), color, tex_coords.top_right());
        draw_data.push_vertex(rect.bottom_right(), color, tex_coords.bottom_right());
        draw_data.push_vertex(rect.bottom_left(), color, tex_coords.bottom_left());

        // top left -> top right -> bottom right
        draw_data.push_triangle(idx + 0, idx + 1, idx + 2);
        // bottom right -> bottom left -> top left
        draw_data.push_triangle(idx + 2, idx + 3, idx + 0);
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
        let draw_data = self.draw_data_mut();
        draw_data.set_sdf_params(None);

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
        let Stroke { width, brush, .. } = line_shape.stroke;
        let offset = compute_line_width_offset(a, b, width);
        self.push_rect_filled(Rect::new(a + offset, b - offset), brush);
    }

    pub fn push_rect(&mut self, rect_shape: RectShape) {
        let RectShape {
            mut rect,
            brush,
            stroke,
            corner_radius,
        } = rect_shape;
        let maybe_sdf_params = match (stroke, corner_radius) {
            (None, None) => None,
            (stroke, corner_radius) => {
                if let Some(ref stroke) = stroke {
                    rect = match stroke.alignment {
                        StrokeAlignment::Inside => rect,
                        StrokeAlignment::Outside => rect.inflate(Vec2::splat(stroke.width)),
                        StrokeAlignment::Center => rect.inflate(Vec2::splat(stroke.width * 0.5)),
                    };
                }
                Some(SdfParams::Rect(RectSdf {
                    center: rect_shape.rect.center(),
                    size: rect_shape.rect.size(),
                    corner_radius,
                    stroke,
                }))
            }
        };

        let draw_data = self.draw_data_mut();
        draw_data.set_sdf_params(maybe_sdf_params);

        self.push_rect_filled(rect, brush.unwrap_or(Brush::Solid(Rgba8::TRANSPARENT)));
    }

    pub fn clear(&mut self) {
        for layer in &mut self.layers {
            layer.clear();
        }
    }

    pub fn drain_layers<'a>(&'a mut self) -> DrawLayersDrain<'a> {
        DrawLayersDrain {
            ptr: unsafe { NonNull::new_unchecked(self) },
            iter: self.layers.iter_mut(),
        }
    }
}
