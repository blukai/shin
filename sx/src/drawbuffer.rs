use std::ops::Range;
use std::ptr::NonNull;
use std::slice;
use std::vec::Drain;

use mars::scopeguard::ScopeGuard;

use crate::{Rect, Rgba8, ShaderUniformValue, ShaderUniforms, TextureHandle, Vec2};

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
    pub corner_radius: Option<f32>,
}

impl RectShape {
    pub fn new(coords: Rect) -> Self {
        Self {
            coords,
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

#[derive(Debug)]
pub struct DrawCommand {
    pub index_range: Range<u32>,
    pub uniforms: Option<ShaderUniforms>,
    pub scissor: Option<Rect>,
}

#[derive(Debug, Default)]
pub struct DrawData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub commands: Vec<DrawCommand>,

    pending_indices: usize,
    active_uniforms: Option<ShaderUniforms>,
    active_scissor: Option<Rect>,
}

impl DrawData {
    fn clear(&mut self) {
        assert_eq!(self.pending_indices, 0);

        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();

        self.active_uniforms = None;
        self.active_scissor = None;
    }

    fn flush(&mut self) {
        if self.pending_indices == 0 {
            return;
        }

        let start_index = (self.indices.len() - self.pending_indices) as u32;
        let end_index = self.indices.len() as u32;

        self.commands.push(DrawCommand {
            index_range: start_index..end_index,
            uniforms: self.active_uniforms.clone(),
            scissor: self.active_scissor,
        });

        self.pending_indices = 0;
    }

    fn set_uniforms(&mut self, uniforms: Option<ShaderUniforms>) {
        if self.active_uniforms == uniforms {
            return;
        }
        self.flush();
        self.active_uniforms = uniforms;
    }

    fn set_scissor(&mut self, scissor: Option<Rect>) {
        if self.active_scissor == scissor {
            return;
        }
        self.flush();
        self.active_scissor = scissor;
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

pub struct DrawLayerFlush<'a> {
    pub vertices: &'a [Vertex],
    pub indices: &'a [u32],
    // drain so that you can take ownership of values
    pub commands: Drain<'a, DrawCommand>,
}

pub struct DrawLayersDrain<'a> {
    iter: slice::IterMut<'a, DrawData>,
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

    fn push_rect_filled(&mut self, coords: Rect, fill: Fill, maybe_corner_radius: Option<f32>) {
        let draw_data = self.layer_mut();
        let idx = draw_data.vertices.len() as u32;

        let mut uniforms = None::<ShaderUniforms>;
        if let Some(ref fill_texture) = fill.texture {
            use ShaderUniformValue as Value;
            let u = uniforms.get_or_insert(ShaderUniforms::default());
            u.set("u_texture", Value::Texture2D(fill_texture.texture))
        }
        if let Some(corner_radius) = maybe_corner_radius {
            use ShaderUniformValue as Value;
            let u = uniforms.get_or_insert(ShaderUniforms::default());
            // TODO: uniform structs.
            u.set("u_rect_center", Value::Vec2(coords.center()));
            u.set("u_rect_half_size", Value::Vec2(coords.size() * 0.5));
            u.set("u_rect_corner_radius", Value::Float(corner_radius));
        }
        draw_data.set_uniforms(uniforms);

        let tex_coords = fill
            .texture
            .map(|texture| texture.coords)
            .unwrap_or(Rect::new(Vec2::splat(0.0), Vec2::splat(1.0)));
        let color = fill.color;

        // top left
        draw_data.push_vertex(Vertex {
            position: coords.top_left(),
            tex_coord: tex_coords.top_left(),
            color,
        });
        // top right
        draw_data.push_vertex(Vertex {
            position: coords.top_right(),
            tex_coord: tex_coords.top_right(),
            color,
        });
        // bottom right
        draw_data.push_vertex(Vertex {
            position: coords.bottom_right(),
            tex_coord: tex_coords.bottom_right(),
            color,
        });
        // bottom left
        draw_data.push_vertex(Vertex {
            position: coords.bottom_left(),
            tex_coord: tex_coords.bottom_left(),
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
        self.push_rect_filled(Rect::new(a + offset, b - offset), Fill::new(color), None);
    }

    fn push_rect_stroked(&mut self, coords: Rect, stroke: Stroke) {
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
        //   expand to left and right
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
        //   shrink top and bottom
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
            assert!(rect.corner_radius.is_none(), "TODO: rounded rect outlines");
            self.push_rect_stroked(rect.coords, stroke);
        }
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
