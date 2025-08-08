use std::{array, mem, ops::Range};

use scopeguard::ScopeGuard;

use crate::{Externs, Rect, TextureKind, Vec2};

// TODO: consider offloading vertex generation and stuff for the gpu (or maybe for software
// renderer?) to the renderer. accumulate shapes, not verticies.

// NOTE: repr(C) is here to ensure that ordering is correct in into_array transmutation.
#[repr(C)]
// NOTE: Copy is derived simply because it's cheap. size of u32.
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
    pub const fn from_bytes(arr: [u8; 4]) -> Self {
        unsafe { mem::transmute(arr) }
    }

    /// works with hex: `Rgba8::from_u32(0x8faf9fff)`.
    #[inline]
    pub const fn from_u32(value: u32) -> Self {
        Self::from_bytes(u32::to_be_bytes(value))
    }
}

#[derive(Debug, Clone)]
pub struct FillTexture<E: Externs> {
    pub kind: TextureKind<E>,
    pub coords: Rect,
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
    pub clip_rect: Option<Rect>,
    pub index_range: Range<u32>,
    pub texture: Option<TextureKind<E>>,
}

#[derive(Debug)]
pub struct DrawData<E: Externs> {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pending_indices: usize,
    pub commands: Vec<DrawCommand<E>>,
}

// @BlindDerive
impl<E: Externs> Default for DrawData<E> {
    fn default() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            pending_indices: 0,
            commands: Vec::new(),
        }
    }
}

impl<E: Externs> DrawData<E> {
    fn clear(&mut self) {
        assert_eq!(self.pending_indices, 0);
        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();
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

    fn commit_primitive(&mut self, clip_rect: Option<Rect>, texture: Option<TextureKind<E>>) {
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

#[repr(usize)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DrawLayer {
    Underlay = 0,
    #[default]
    Primary = 1,
}

impl DrawLayer {
    pub const MAX: usize = 2;
}

#[derive(Debug)]
pub struct DrawBuffer<E: Externs> {
    clip_rect: Option<Rect>,
    layers: [DrawData<E>; DrawLayer::MAX],
    layer: DrawLayer,
}

// @BlindDerive
impl<E: Externs> Default for DrawBuffer<E> {
    fn default() -> Self {
        Self {
            clip_rect: None,
            layers: array::from_fn(|_| DrawData::default()),
            layer: DrawLayer::default(),
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
        self.clip_rect = Some(rect);
        ScopeGuard::new_with_data(self, |this| this.clip_rect = None)
    }

    pub fn layer_scope<'a>(
        &'a mut self,
        layer: DrawLayer,
    ) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)> {
        let layer_backup = self.layer;
        self.layer = layer;
        ScopeGuard::new_with_data(self, move |this| this.layer = layer_backup)
    }

    pub fn iter_draw_data<'a>(&'a self) -> impl Iterator<Item = &'a DrawData<E>> {
        self.layers.iter()
    }

    #[inline(always)]
    fn get_draw_data_mut(&mut self) -> &mut DrawData<E> {
        &mut self.layers[self.layer as usize]
    }

    pub fn push_line(&mut self, line: LineShape) {
        // NOTE: line's stroke may only be centered. specifying outside/inside stroke alignment
        // makes sense only for shapes that need an outline (/ need to be stroked).
        assert!(matches!(line.stroke.alignment, StrokeAlignment::Center));

        let clip_rect = self.clip_rect;
        let draw_data = self.get_draw_data_mut();
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
        let draw_data = self.get_draw_data_mut();
        let idx = draw_data.vertices.len() as u32;

        let (color, texture, tex_coords) = if let Some(fill_texture) = fill.texture {
            (
                fill.color,
                Some(fill_texture.kind),
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
            StrokeAlignment::Inside => coords.shrink(&Vec2::splat(half_width)),
            StrokeAlignment::Outside => coords.expand(&Vec2::splat(half_width)),
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
}
