use std::ops::Range;
use std::ptr::NonNull;
use std::slice;

use mars::{
    alloc,
    array::{Drain, GrowableArray},
    arraymemory::GrowableArrayMemory,
};

use crate::{Rect, Rgba8, TextureHandle, Vec2};

// TODO: when doing any kind of sdf - fill the entire screen with a single triangle or quad no
// matter what the thing is.

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UniformValue {
    Int(i32),
    Int2([i32; 2]),
    Int3([i32; 3]),
    Int4([i32; 4]),
    Float(f32),
    Float2([f32; 2]),
    Float3([f32; 3]),
    Float4([f32; 4]),
    Mat4([[f32; 4]; 4]),
}

#[derive(Debug)]
pub struct DrawCommand {
    pub index_range: Range<u32>,
    pub uniform_block: Option<Range<u32>>,
    // TODO: primitive?
    pub texture: Option<TextureHandle>,
    pub scissor: Option<Rect>,
}

#[derive(Debug, Default)]
pub struct DrawLayerData {
    pub vertices: GrowableArray<Vertex, alloc::Global>,
    pub indices: GrowableArray<u32, alloc::Global>,
    pub uniforms: GrowableArray<UniformValue, alloc::Global>,
    pending_indices: u32,
    current_uniform_block: Option<Range<u32>>,
    current_texture: Option<TextureHandle>,
    current_scissor: Option<Rect>,
    pub commands: GrowableArray<DrawCommand, alloc::Global>,
}

impl DrawLayerData {
    fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.uniforms.clear();
        assert_eq!(self.pending_indices, 0);
        self.current_uniform_block = None;
        self.current_texture = None;
        self.current_scissor = None;
        self.commands.clear();
    }

    fn flush(&mut self) {
        if self.pending_indices == 0 {
            return;
        }

        let end_index = self.indices.len() as u32;
        let start_index = end_index - self.pending_indices;
        self.commands.push(DrawCommand {
            index_range: start_index..end_index,
            uniform_block: self.current_uniform_block.clone(),
            texture: self.current_texture,
            scissor: self.current_scissor,
        });

        self.pending_indices = 0;
    }

    fn set_uniform_block<const N: usize>(&mut self, uniform_block: Option<[UniformValue; N]>) {
        match (self.current_uniform_block.clone(), uniform_block) {
            (None, None) => {}
            (Some(..), None) => {
                self.flush();
                self.current_uniform_block = None;
            }
            (None, Some(next_values)) => {
                self.flush();
                let start = self.uniforms.len() as u32;
                // NOCOMMIT
                self.uniforms.extend_from_iter(next_values.into_iter());
                let end = self.uniforms.len() as u32;
                self.current_uniform_block = Some(start..end);
            }
            (Some(prev_value_range), Some(next_values)) => {
                let prev_values =
                    &self.uniforms[prev_value_range.start as usize..prev_value_range.end as usize];
                if prev_values == next_values {
                    return;
                }

                self.flush();
                let start = self.uniforms.len() as u32;
                // NOCOMMIT
                self.uniforms.extend_from_iter(next_values.into_iter());
                let end = self.uniforms.len() as u32;
                self.current_uniform_block = Some(start..end);
            }
        }
    }

    fn set_texture(&mut self, texture: Option<TextureHandle>) {
        if self.current_texture == texture {
            return;
        }
        self.flush();
        self.current_texture = texture;
    }

    fn set_scissor_rect(&mut self, rect: Option<Rect>) {
        let prev = self.current_scissor;
        let next = match (prev, rect) {
            (Some(prev), Some(next)) => Some(next.clamp(prev)),
            (_, next) => next,
        };
        if prev == next {
            return;
        }
        self.flush();
        self.current_scissor = next;
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
    pub uniforms: &'a [UniformValue],
    // drain so that you can take ownership of values
    //
    // TODO: can you type-erase it? it'll be very annoying and dumb to have to parametrize flush,
    // drain thing with an allocator.
    pub commands: Drain<'a, DrawCommand, GrowableArrayMemory<DrawCommand, alloc::Global>>,
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
                uniforms: layer.uniforms.as_slice(),
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
        self.draw_data().current_scissor
    }

    pub fn set_scissor_rect(&mut self, rect: Option<Rect>) {
        self.draw_data_mut().set_scissor_rect(rect)
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

    // TODO
    // fn push_rect_stroked(&mut self, rect: Rect, stroke: Stroke) {
    //     todo!()
    // }

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
        draw_data.set_uniform_block::<0>(None);

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
        let draw_data = self.draw_data_mut();

        let RectShape {
            mut rect,
            fill: maybe_fill,
            stroke: maybe_stroke,
            corner_radius: maybe_corner_radius,
        } = rect_shape;
        match (maybe_stroke, maybe_corner_radius) {
            (None, None) => {
                draw_data.set_uniform_block::<0>(None);
            }
            (maybe_stroke, maybe_corner_radius) => {
                let center = rect.center().to_array();
                let size = rect.size().to_array();
                let corner_radius = maybe_corner_radius.unwrap_or(0.0);
                let mut stroke_width = 0.0;
                let mut stroke_color = [0f32; 4];
                let mut stroke_alignment = 0i32;

                if let Some(stroke) = maybe_stroke {
                    stroke_width = stroke.width;
                    stroke_color = stroke.color.to_f32_array();
                    match stroke.alignment {
                        StrokeAlignment::Inside => {
                            stroke_alignment = -1;
                        }
                        // NOTE: in cases of outside/center outline rect needs to be scaled up.
                        // this does not change size of the rect, no; but "reserves" space for the
                        // outline.
                        StrokeAlignment::Outside => {
                            stroke_alignment = 1;

                            rect = rect.inflate(Vec2::splat(stroke.width));
                        }
                        StrokeAlignment::Center => {
                            stroke_alignment = 0;

                            rect = rect.inflate(Vec2::splat(stroke.width * 0.5));
                        }
                    }
                }

                // NOTE: order must match the shader.
                draw_data.set_uniform_block(Some([
                    UniformValue::Float2(center),
                    UniformValue::Float2(size),
                    UniformValue::Float(corner_radius),
                    UniformValue::Float(stroke_width),
                    UniformValue::Float4(stroke_color),
                    UniformValue::Int(stroke_alignment),
                ]));
            }
        }

        self.push_rect_filled(rect, maybe_fill.unwrap_or(Fill::Color(Rgba8::TRANSPARENT)));
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
