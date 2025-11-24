use std::ops::Range;
use std::ptr::NonNull;
use std::slice;

use mars::scopeguard::ScopeGuard;

use crate::{Rect, Rgba8, TextureHandle, Vec2};

// TODO: consider offloading vertex generation and stuff to the gpu
//   (or maybe for software renderer?) to the renderer.
//   maybe accumulate shapes, not verticies.

#[derive(Debug, Clone)]
pub struct FillTexture {
    pub texture: TextureHandle,
    pub coords: Rect,
}

impl FillTexture {
    pub fn new(texture: TextureHandle, coords: Rect) -> Self {
        Self { texture, coords }
    }
}

#[derive(Debug, Clone)]
pub struct Fill {
    pub color: Rgba8,
    pub texture: Option<FillTexture>,
}

impl Fill {
    pub fn new(color: Rgba8) -> Self {
        Self {
            color,
            texture: None,
        }
    }

    pub fn with_texture(mut self, texture: Option<FillTexture>) -> Self {
        self.texture = texture;
        self
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub enum StrokeAlignment {
    Inside,
    Outside,
    #[default]
    Center,
}

#[derive(Debug, Clone)]
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
    pub coords: Rect,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    pub corner_radius: f32,
}

impl RectShape {
    pub fn new(coords: Rect) -> Self {
        Self {
            coords,
            fill: None,
            stroke: None,
            corner_radius: 0.0,
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

    pub fn with_corner_radius(mut self, corner_radius: f32) -> Self {
        self.corner_radius = corner_radius;
        self
    }
}

#[derive(Debug)]
pub struct LineShape {
    pub points: [Vec2; 2],
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

/// computes the vertex position offset away the from center caused by line width.
fn compute_line_width_offset(a: Vec2, b: Vec2, width: f32) -> Vec2 {
    // direction defines how the line is oriented in space. it allows to know
    // which way to move the vertices to create the desired thickness.
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

// TODO: instancing (to enable batching (vertices will be able to exist in 0..1 coordinate space
// (probably) and then they can be translated, scaled, rotated with instance transforms (for
// example this will allow to render all rects within a single draw call? or am i being
// delusional?))).

#[repr(C)]
#[derive(Debug)]
pub struct Vertex {
    /// screen pixel coordinates.
    /// 0, 0 is the top left corner of the screen.
    pub position: Vec2,
    /// normalized texture coordinates.
    /// 0, 0 is the top left corner of the texture.
    /// 1, 1 is the bottom right corner of the texture.
    pub tex_coord: Vec2,
    pub color: Rgba8,
}

// TODO: shader service or something?
//   user must be able to provide custom shaders
//   it must be possible to provide custom params(uniforms).
//     maybe take a look at fyrox's PropertyGroup thing.

#[derive(Debug, Clone, PartialEq)]
pub enum DrawParams {
    RoundedRect {
        center: Vec2,
        half_size: Vec2,
        // TODO: radii: [f32; 4],
        corner_radius: f32,
    },
}

#[derive(Debug)]
pub struct DrawCommand {
    // TODO: can index range span many shapes that share same bindings (same texture)?
    pub index_range: Range<u32>,
    pub texture: Option<TextureHandle>,
    pub scissor: Option<Rect>,
    pub params: Option<DrawParams>,
}

#[derive(Debug, Default)]
pub struct DrawData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub commands: Vec<DrawCommand>,

    pending_indices: usize,
    active_texture: Option<TextureHandle>,
    active_scissor: Option<Rect>,
    active_params: Option<DrawParams>,
}

impl DrawData {
    fn clear(&mut self) {
        assert_eq!(self.pending_indices, 0);

        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();

        self.active_texture = None;
        self.active_scissor = None;
        self.active_params = None;
    }

    fn flush(&mut self) {
        if self.pending_indices == 0 {
            return;
        }

        let start_index = (self.indices.len() - self.pending_indices) as u32;
        let end_index = self.indices.len() as u32;
        self.commands.push(DrawCommand {
            index_range: start_index..end_index,
            texture: self.active_texture,
            scissor: self.active_scissor,
            params: self.active_params.clone(),
        });

        self.pending_indices = 0;
    }

    fn set_texture(&mut self, texture: Option<TextureHandle>) {
        if self.active_texture == texture {
            return;
        }
        self.flush();
        self.active_texture = texture;
    }

    fn set_scissor(&mut self, scissor: Option<Rect>) {
        if self.active_scissor == scissor {
            return;
        }
        self.flush();
        self.active_scissor = scissor;
    }

    fn set_params(&mut self, params: Option<DrawParams>) {
        if self.active_params == params {
            return;
        }
        self.flush();
        self.active_params = params;
    }

    fn push_vertex(&mut self, vertex: Vertex) {
        self.vertices.push(vertex);
    }

    fn push_triangle(&mut self, zero: u32, ichi: u32, ni: u32) {
        self.indices.push(zero);
        self.indices.push(ichi);
        self.indices.push(ni);
        self.pending_indices += 3;
    }
}

pub struct DrawLayerDrain<'a> {
    iter: slice::IterMut<'a, DrawData>,
    ptr: NonNull<DrawBuffer>,
}

impl<'a> Iterator for DrawLayerDrain<'a> {
    type Item = &'a DrawData;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|layer| {
            layer.flush();
            let layer: &_ = layer;
            layer
        })
    }
}

impl<'a> Drop for DrawLayerDrain<'a> {
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
    Base,
}

impl DrawLayer {
    pub const MAX: usize = 1;
}

#[derive(Debug, Default)]
pub struct DrawBuffer {
    layer: DrawLayer,
    layers: [DrawData; DrawLayer::MAX],
}

impl DrawBuffer {
    #[inline(always)]
    fn layer_mut(&mut self) -> &mut DrawData {
        &mut self.layers[self.layer as usize]
    }

    pub fn scissor_scope_guard<'a>(
        &'a mut self,
        rect: Rect,
    ) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)> {
        let draw_data = self.layer_mut();

        let prev = draw_data.active_scissor;
        let next = if let Some(prev) = prev {
            rect.clamp(prev)
        } else {
            rect
        };

        draw_data.set_scissor(Some(next));

        let layer = self.layer;
        ScopeGuard::new_with_data(self, move |this| {
            this.layers[layer as usize].set_scissor(prev);
        })
    }

    pub fn layer_scope_guard<'a>(
        &'a mut self,
        layer: DrawLayer,
    ) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)> {
        let prev = self.layer;
        self.layer = layer;
        ScopeGuard::new_with_data(self, move |this| this.layer = prev)
    }

    fn push_rect_filled(&mut self, coords: Rect, fill: Fill, corner_radius: f32) {
        let draw_data = self.layer_mut();
        let idx = draw_data.vertices.len() as u32;

        let (color, texture, tex_coords) = if let Some(fill_texture) = fill.texture {
            (
                fill.color,
                Some(fill_texture.texture),
                Some(fill_texture.coords),
            )
        } else {
            (fill.color, None, None)
        };
        draw_data.set_texture(texture);

        let params = if corner_radius > 0.0 {
            Some(DrawParams::RoundedRect {
                center: coords.center(),
                half_size: coords.size() * 0.5,
                corner_radius,
            })
        } else {
            None
        };
        draw_data.set_params(params);

        // top left
        draw_data.push_vertex(Vertex {
            position: coords.top_left(),
            tex_coord: tex_coords
                .as_ref()
                .map(|tc| tc.top_left())
                .unwrap_or(Vec2::new(0.0, 0.0)),
            color,
        });
        // top right
        draw_data.push_vertex(Vertex {
            position: coords.top_right(),
            tex_coord: tex_coords
                .as_ref()
                .map(|tc| tc.top_right())
                .unwrap_or(Vec2::new(1.0, 0.0)),
            color,
        });
        // bottom right
        draw_data.push_vertex(Vertex {
            position: coords.bottom_right(),
            tex_coord: tex_coords
                .as_ref()
                .map(|tc| tc.bottom_right())
                .unwrap_or(Vec2::new(1.0, 1.0)),
            color,
        });
        // bottom left
        draw_data.push_vertex(Vertex {
            position: coords.bottom_left(),
            tex_coord: tex_coords
                .as_ref()
                .map(|tc| tc.bottom_left())
                .unwrap_or(Vec2::new(0.0, 1.0)),
            color,
        });

        // top left -> top right -> bottom right
        draw_data.push_triangle(idx + 0, idx + 1, idx + 2);
        // bottom right -> bottom left -> top left
        draw_data.push_triangle(idx + 2, idx + 3, idx + 0);
    }

    pub fn push_line(&mut self, line: LineShape) {
        // NOTE: line's stroke may only be centered.
        //   specifying outside/inside stroke alignment makes sense only for shapes that need an
        //   outline (/ need to be stroked).
        assert!(matches!(line.stroke.alignment, StrokeAlignment::Center));

        let [a, b] = line.points;
        let Stroke { width, color, .. } = line.stroke;
        let offset = compute_line_width_offset(a, b, width);
        self.push_rect_filled(Rect::new(a + offset, b - offset), Fill::new(color), 0.0);
    }

    fn push_rect_stroked(&mut self, coords: Rect, stroke: Stroke, corner_radius: f32) {
        assert_eq!(corner_radius, 0.0, "can't do stroke rounded rects");

        let half_width = stroke.width * 0.5;
        let coords = match stroke.alignment {
            StrokeAlignment::Inside => coords.inflate(-Vec2::splat(half_width)),
            StrokeAlignment::Outside => coords.inflate(Vec2::splat(half_width)),
            StrokeAlignment::Center => coords,
        };
        let top_left = coords.top_left();
        let top_right = coords.top_right();
        let bottom_right = coords.bottom_right();
        let bottom_left = coords.bottom_left();

        let stroke = Stroke {
            alignment: StrokeAlignment::Center,
            ..stroke
        };

        // horizontal lines:
        // expand to left and right
        self.push_line(LineShape::new(
            Vec2::new(top_left.x - half_width, top_left.y),
            Vec2::new(top_right.x + half_width, top_right.y),
            stroke.clone(),
        ));
        self.push_line(LineShape::new(
            Vec2::new(bottom_left.x - half_width, bottom_left.y),
            Vec2::new(bottom_right.x + half_width, bottom_right.y),
            stroke.clone(),
        ));

        // vertical lines:
        // shrink top and bottom
        self.push_line(LineShape::new(
            Vec2::new(top_right.x, top_right.y + half_width),
            Vec2::new(bottom_right.x, bottom_right.y - half_width),
            stroke.clone(),
        ));
        self.push_line(LineShape::new(
            Vec2::new(top_left.x, top_left.y + half_width),
            Vec2::new(bottom_left.x, bottom_left.y - half_width),
            stroke,
        ));
    }

    pub fn push_rect(&mut self, rect: RectShape) {
        if let Some(fill) = rect.fill {
            self.push_rect_filled(rect.coords, fill, rect.corner_radius);
        }
        if let Some(stroke) = rect.stroke {
            self.push_rect_stroked(rect.coords, stroke, rect.corner_radius);
        }
    }

    pub fn clear(&mut self) {
        for layer in &mut self.layers {
            layer.clear();
        }
    }

    pub fn drain_layers<'a>(&'a mut self) -> DrawLayerDrain<'a> {
        DrawLayerDrain {
            ptr: unsafe { NonNull::new_unchecked(self) },
            iter: self.layers.iter_mut(),
        }
    }
}
