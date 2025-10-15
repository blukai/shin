use std::ops::Range;
use std::{array, mem};

use scopeguard::ScopeGuard;

use crate::{Externs, Rect, TextureHandle, TextureHandleKind, Vec2};

// TODO: consider offloading vertex generation and stuff for the gpu (or maybe for software
// renderer?) to the renderer. accumulate shapes, not verticies.

// NOTE: repr(C) is here to ensure that ordering is correct in into_array transmutation.
// NOTE: Copy is derived simply because it's cheap. size of Rgba == size of u32.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    // https://en.wikipedia.org/wiki/Web_colors#Basic_colors
    pub const WHITE: Self = Self::new(255, 255, 255, 255);
    pub const SILVER: Self = Self::new(192, 192, 192, 255);
    pub const GRAY: Self = Self::new(128, 128, 128, 255);
    pub const BLACK: Self = Self::new(0, 0, 0, 255);
    pub const RED: Self = Self::new(255, 0, 0, 255);
    pub const MAROON: Self = Self::new(128, 0, 0, 255);
    pub const YELLOW: Self = Self::new(255, 255, 0, 255);
    pub const OLIVE: Self = Self::new(128, 128, 0, 255);
    pub const LIME: Self = Self::new(0, 255, 0, 255);
    pub const GREEN: Self = Self::new(0, 128, 0, 255);
    pub const AQUA: Self = Self::new(0, 255, 255, 255);
    pub const TEAL: Self = Self::new(0, 128, 128, 255);
    pub const BLUE: Self = Self::new(0, 0, 255, 255);
    pub const NAVY: Self = Self::new(0, 0, 128, 255);
    pub const FUCHSIA: Self = Self::new(255, 0, 255, 255);
    pub const PURPLE: Self = Self::new(128, 0, 128, 255);
    // the missing rubik's cube color xd
    pub const ORANGE: Self = Self::new(255, 165, 0, 255);

    #[inline]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn from_u8_array(arr: [u8; 4]) -> Self {
        unsafe { mem::transmute(arr) }
    }

    /// works with hex: `Rgba8::from_u32(0x8faf9fff)`.
    #[inline]
    pub const fn from_u32(value: u32) -> Self {
        Self::from_u8_array(u32::to_be_bytes(value))
    }

    #[inline]
    pub const fn from_f32_array(arr: [f32; 4]) -> Self {
        Self::from_u8_array([
            (arr[0].clamp(0.0, 1.0) * 255.0) as u8,
            (arr[1].clamp(0.0, 1.0) * 255.0) as u8,
            (arr[2].clamp(0.0, 1.0) * 255.0) as u8,
            (arr[3].clamp(0.0, 1.0) * 255.0) as u8,
        ])
    }

    #[inline]
    pub const fn to_f32_array(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }

    pub const fn with_a(mut self, a: u8) -> Self {
        self.a = a;
        self
    }

    pub const fn with_af(mut self, a: f32) -> Self {
        debug_assert!(a >= 0.0 && a <= 1.0);
        self.a = (a * 255 as f32) as u8;
        self
    }
}

#[test]
fn test_rgba8_f32_conversions() {
    let arru8 = [255, 0, 0, 255];
    let arrf32 = [1.0, 0.0, 0.0, 1.0];
    assert_eq!(Rgba8::from_u8_array(arru8).to_f32_array(), arrf32);
    assert_eq!(Rgba8::from_f32_array(arrf32), Rgba8::from_u8_array(arru8));
}

#[derive(Debug, Clone)]
pub struct FillTexture<E: Externs> {
    pub texture: TextureHandleKind<E>,
    pub coords: Rect,
}

impl<E: Externs> FillTexture<E> {
    pub fn new(texture: TextureHandleKind<E>, coords: Rect) -> Self {
        Self { texture, coords }
    }

    pub fn new_internal(texture: TextureHandle, coords: Rect) -> Self {
        Self::new(TextureHandleKind::Internal(texture), coords)
    }

    pub fn new_external(texture: E::TextureHandle, coords: Rect) -> Self {
        Self::new(TextureHandleKind::External(texture), coords)
    }
}

#[derive(Debug, Clone)]
pub struct Fill<E: Externs> {
    pub color: Rgba8,
    pub texture: Option<FillTexture<E>>,
}

impl<E: Externs> Fill<E> {
    pub fn new(color: Rgba8, texture: FillTexture<E>) -> Self {
        Self {
            color,
            texture: Some(texture),
        }
    }

    pub fn new_with_color(color: Rgba8) -> Self {
        Self {
            color,
            texture: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum StrokeAlignment {
    Inside,
    Outside,
    Center,
}

#[derive(Debug, Clone)]
pub struct Stroke {
    pub width: f32,
    pub color: Rgba8,
    pub alignment: StrokeAlignment,
}

impl Stroke {
    // NOTE: in majority of cases alignment is `Center`.
    pub fn new(width: f32, color: Rgba8) -> Self {
        Self {
            width,
            color,
            alignment: StrokeAlignment::Center,
        }
    }

    pub fn with_alignment(mut self, alignment: StrokeAlignment) -> Self {
        self.alignment = alignment;
        self
    }
}

#[derive(Debug)]
pub struct RectShape<E: Externs> {
    pub coords: Rect,
    pub fill: Option<Fill<E>>,
    pub stroke: Option<Stroke>,
}

impl<E: Externs> RectShape<E> {
    pub fn new(coords: Rect, fill: Fill<E>, stroke: Stroke) -> Self {
        Self {
            coords,
            fill: Some(fill),
            stroke: Some(stroke),
        }
    }

    pub fn new_with_fill(coords: Rect, fill: Fill<E>) -> Self {
        Self {
            coords,
            fill: Some(fill),
            stroke: None,
        }
    }

    pub fn new_with_stroke(coords: Rect, stroke: Stroke) -> Self {
        Self {
            coords,
            fill: None,
            stroke: Some(stroke),
        }
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
pub struct DrawCommand<E: Externs> {
    // TODO: maybe don't store clip rect within the command, but instead make it possible to group
    // commands with attributes or something.
    pub clip_rect: Option<Rect>,
    pub index_range: Range<u32>,
    pub texture: Option<TextureHandleKind<E>>,
}

// TODO: split DrawData into buffers and commands
//
//   bounds accumulator will need to live next to commands.
//
//   when lending new draw buffers make sure that the lended draw buffer will be pushing indices
//   and vertices into central buffers that it'll be referencing (possibly will need an Rc and a
//   RefCell, but i really don't want an Rc).
//
//   OR maybe there needs to be a centeral draw buffer only for storage / allocation; it'll not be
//   possible to push shapes into it. its purpose will be to hand-out other kind of draw buffer
//   that'll be for drawing.
//   not 100%, but i think this will make it easier for something higher order to start scopes
//   (clip, layer, etc.).
//   but perhaps RefCell + RefMut::map_split can do well here; and ownership semantics will be
//   clear.
#[derive(Debug)]
pub struct DrawData<E: Externs> {
    pub vertices: Vec<Vertex>,
    bounds: Rect,
    pub indices: Vec<u32>,
    pending_indices: usize,
    pub commands: Vec<DrawCommand<E>>,
}

// @BlindDerive
impl<E: Externs> Default for DrawData<E> {
    fn default() -> Self {
        Self {
            vertices: Vec::new(),
            bounds: Rect::new(Vec2::ZERO, Vec2::ZERO),
            indices: Vec::new(),
            pending_indices: 0,
            commands: Vec::new(),
        }
    }
}

impl<E: Externs> DrawData<E> {
    fn clear(&mut self) {
        self.vertices.clear();
        self.bounds = Rect::new(Vec2::ZERO, Vec2::ZERO);
        self.indices.clear();
        assert_eq!(self.pending_indices, 0);
        self.commands.clear();
    }

    fn push_vertex(&mut self, vertex: Vertex) {
        if self.vertices.is_empty() {
            self.bounds.min = vertex.position;
            self.bounds.max = vertex.position;
        } else {
            self.bounds.min = self.bounds.min.min(vertex.position);
            self.bounds.max = self.bounds.max.max(vertex.position);
        }
        self.vertices.push(vertex);
    }

    fn push_triangle(&mut self, zero: u32, ichi: u32, ni: u32) {
        self.indices.push(zero);
        self.indices.push(ichi);
        self.indices.push(ni);
        self.pending_indices += 3;
    }

    fn commit_primitive(&mut self, clip_rect: Option<Rect>, texture: Option<TextureHandleKind<E>>) {
        assert!(self.pending_indices > 0);
        let start_index = (self.indices.len() - self.pending_indices) as u32;
        let end_index = self.indices.len() as u32;
        self.commands.push(DrawCommand {
            clip_rect,
            index_range: start_index..end_index,
            texture,
        });
        self.pending_indices = 0;
    }
}

// NOTE: the initial idea for why i did implement this didn't work out, but it doesn't mean that
// the implementation is completely useless. this will probably work pretty well for tooptips and
// stuff.
#[repr(usize)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DrawLayer {
    #[default]
    Primary,
}

impl DrawLayer {
    pub const MAX: usize = 1;
}

#[derive(Debug)]
pub struct DrawBuffer<E: Externs> {
    clip_rect: Option<Rect>,
    layer: DrawLayer,
    layers: [DrawData<E>; DrawLayer::MAX],
    stagers: Vec<Self>,
}

// @BlindDerive
impl<E: Externs> Default for DrawBuffer<E> {
    fn default() -> Self {
        Self {
            clip_rect: None,
            layer: DrawLayer::default(),
            layers: array::from_fn(|_| DrawData::default()),
            stagers: Vec::default(),
        }
    }
}

impl<E: Externs> DrawBuffer<E> {
    pub fn clear(&mut self) {
        self.layers
            .iter_mut()
            .for_each(|draw_data| draw_data.clear());
    }

    pub fn clip_scope<'a>(
        &'a mut self,
        rect: Rect,
    ) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)> {
        let next = if let Some(prev) = self.clip_rect {
            rect.clamp(prev)
        } else {
            rect
        };
        let prev = self.clip_rect.replace(next);
        ScopeGuard::new_with_data(self, move |this| this.clip_rect = prev)
    }

    pub fn layer_scope<'a>(
        &'a mut self,
        layer: DrawLayer,
    ) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)> {
        let prev = self.layer;
        self.layer = layer;
        ScopeGuard::new_with_data(self, move |this| this.layer = prev)
    }

    /// returns an instance of self with inherited clip rect and layer.
    pub fn lend<const N: usize>(&mut self) -> [Self; N] {
        array::from_fn(|_| {
            let mut ret = self.stagers.pop().unwrap_or_else(|| Self::default());
            ret.clear();
            ret.clip_rect = self.clip_rect;
            ret.layer = self.layer;
            ret
        })
    }

    /// extends self from other instance that most likely was handed-out by [`Self::lend`].
    /// the other instance will not be discarded, but will be stored for future lending.
    pub fn extend<I: IntoIterator<Item = Self>>(&mut self, others: I) {
        // TODO: central allocator / storage
        //   see other TODO comment next to DrawData struct.

        for mut other in others {
            for (src, dst) in other.layers.iter_mut().zip(self.layers.iter_mut()) {
                let base_vertex = dst.vertices.len() as u32;
                let base_index = dst.indices.len() as u32;
                dst.vertices.extend(src.vertices.drain(..));
                dst.indices
                    .extend(src.indices.drain(..).map(|it| it + base_vertex));
                dst.commands
                    .extend(src.commands.drain(..).map(|it| DrawCommand {
                        index_range: it.index_range.start + base_index
                            ..it.index_range.end + base_index,
                        ..it
                    }));
            }
            self.stagers.push(other);
        }
    }

    pub fn stage_scope<'a, const N: usize>(
        &'a mut self,
    ) -> ScopeGuard<[Self; N], impl FnOnce([Self; N])> {
        ScopeGuard::new_with_data(self.lend(), |stagers| self.extend(stagers))
    }

    #[inline(always)]
    fn draw_data(&self) -> &DrawData<E> {
        &self.layers[self.layer as usize]
    }

    #[inline(always)]
    fn draw_data_mut(&mut self) -> &mut DrawData<E> {
        &mut self.layers[self.layer as usize]
    }

    pub fn push_line(&mut self, line: LineShape) {
        // NOTE: line's stroke may only be centered. specifying outside/inside stroke alignment
        // makes sense only for shapes that need an outline (/ need to be stroked).
        assert!(matches!(line.stroke.alignment, StrokeAlignment::Center));

        let clip_rect = self.clip_rect;
        let draw_data = self.draw_data_mut();
        let idx = draw_data.vertices.len() as u32;

        let [a, b] = line.points;
        let Stroke { width, color, .. } = line.stroke;

        let offset = compute_line_width_offset(a, b, width);

        // top left
        draw_data.push_vertex(Vertex {
            position: a + offset,
            tex_coord: Vec2::new(0.0, 0.0),
            color,
        });
        // top right
        draw_data.push_vertex(Vertex {
            position: b + offset,
            tex_coord: Vec2::new(1.0, 0.0),
            color,
        });
        // bottom right
        draw_data.push_vertex(Vertex {
            position: b - offset,
            tex_coord: Vec2::new(1.0, 1.0),
            color,
        });
        // bottom left
        draw_data.push_vertex(Vertex {
            position: a - offset,
            tex_coord: Vec2::new(0.0, 1.0),
            color,
        });

        // top left -> top right -> bottom right
        draw_data.push_triangle(idx + 0, idx + 1, idx + 2);
        // bottom right -> bottom left -> top left
        draw_data.push_triangle(idx + 2, idx + 3, idx + 0);

        draw_data.commit_primitive(clip_rect, None);
    }

    fn push_rect_filled(&mut self, coords: Rect, fill: Fill<E>) {
        let clip_rect = self.clip_rect;
        let draw_data = self.draw_data_mut();
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

        draw_data.commit_primitive(clip_rect, texture);
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

    pub fn push_rect(&mut self, rect: RectShape<E>) {
        if let Some(fill) = rect.fill {
            self.push_rect_filled(rect.coords, fill);
        }
        if let Some(stroke) = rect.stroke {
            self.push_rect_stroked(rect.coords, stroke);
        }
    }

    pub fn bounds(&self) -> Rect {
        self.draw_data().bounds
    }

    // TODO: would it make any sense to offload transforms to gpu
    //   apply this translation in vertex shader?
    //   i imagine this would be very similar to what's happening with clip rect except we'll need
    //   to set a uniform. should be easy.
    //
    // TODO: maybe clarity what this function exactly does
    //   applies translation to all vertices on the current layer.
    //   it can't really be the same as clip_scope, etc because for what i currently want to use
    //   translations i know deltas only after i push shapes into draw buffer.
    pub fn translate(&mut self, delta: Vec2) {
        let draw_data = self.draw_data_mut();
        draw_data.vertices.iter_mut().for_each(|vertex| {
            vertex.position += delta;
        });
        draw_data.bounds = draw_data.bounds.translate(delta);
        draw_data.commands.iter_mut().for_each(|command| {
            if let Some(ref mut clip_rect) = command.clip_rect {
                *clip_rect = clip_rect.translate(delta);
            }
        });
    }

    pub fn iter_draw_data<'a>(&'a self) -> impl Iterator<Item = &'a DrawData<E>> {
        self.layers.iter()
    }
}
