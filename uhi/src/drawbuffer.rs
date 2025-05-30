use std::ops::Range;

use glam::Vec2;

use crate::{Fill, LineShape, Rect, RectShape, Stroke, TextureKind, Vertex, renderer::Renderer};

// TODO: instancing (to enable batching (vertices will be able to exist in 0..1 coordinate space
// (probably) and then they can be translated, scaled, rotated with instance transforms (for
// example this will allow to render all rects within a single draw call? or am i being
// delusional?))).

/// computes the vertex position offset away the from center caused by line width.
fn compute_line_width_offset(a: &Vec2, b: &Vec2, width: f32) -> Vec2 {
    // direction defines how the line is oriented in space. it allows to know
    // which way to move the vertices to create the desired thickness.
    let dir: Vec2 = *b - *a;

    // normalizing the direction vector converts it into a unit vector (length
    // of 1). normalization ensures that the offset is proportional to the line
    // width, regardless of the line's length.
    let norm_dir: Vec2 = dir.normalize_or_zero();

    // create a vector that points outward from the line. we want to move the
    // vertices away from the center of the line, not along its length.
    let perp: Vec2 = norm_dir.perp();

    // to distribute the offset evenly on both sides of the line
    let offset = perp * (width * 0.5);

    offset
}

// TODO: introduce DrawData struct that DrawBuffer would need to expose without exposing its
// internal vertices, indices and draw commands.

// TODO: do Primitive instead of DrawCommand?
#[derive(Debug)]
pub struct DrawCommand<R: Renderer> {
    pub index_range: Range<u32>,
    pub texture: Option<TextureKind<R>>,
}

#[derive(Debug)]
pub struct DrawData<'a, R: Renderer> {
    pub indices: &'a [u32],
    pub vertices: &'a [Vertex],
    pub commands: &'a [DrawCommand<R>],
}

#[derive(Debug)]
pub struct DrawBuffer<R: Renderer> {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    pending_indices: usize,
    draw_commands: Vec<DrawCommand<R>>,
}

impl<R: Renderer> Default for DrawBuffer<R> {
    fn default() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            pending_indices: 0,
            draw_commands: Vec::new(),
        }
    }
}

impl<R: Renderer> DrawBuffer<R> {
    pub fn clear(&mut self) {
        assert!(self.pending_indices == 0);
        self.vertices.clear();
        self.indices.clear();
        self.draw_commands.clear();
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

    fn commit_primitive(&mut self, texture: Option<TextureKind<R>>) {
        if self.pending_indices == 0 {
            return;
        }
        let start_index = (self.indices.len() - self.pending_indices) as u32;
        let end_index = self.indices.len() as u32;
        self.draw_commands.push(DrawCommand {
            index_range: start_index..end_index,
            texture,
        });
        self.pending_indices = 0;
    }

    pub fn get_draw_data<'a>(&'a self) -> DrawData<'a, R> {
        DrawData {
            indices: self.indices.as_slice(),
            vertices: self.vertices.as_slice(),
            commands: self.draw_commands.as_slice(),
        }
    }

    pub fn push_line(&mut self, line: LineShape) {
        let idx = self.vertices.len() as u32;

        let [a, b] = line.points;
        let Stroke { width, color } = line.stroke;

        let perp = compute_line_width_offset(&a, &b, width);

        // top left
        self.push_vertex(Vertex {
            position: a - perp,
            tex_coord: Vec2::new(0.0, 0.0),
            color: color.clone(),
        });
        // top right
        self.push_vertex(Vertex {
            position: b - perp,
            tex_coord: Vec2::new(1.0, 0.0),
            color: color.clone(),
        });
        // bottom right
        self.push_vertex(Vertex {
            position: b + perp,
            tex_coord: Vec2::new(1.0, 1.0),
            color: color.clone(),
        });
        // bottom left
        self.push_vertex(Vertex {
            position: a + perp,
            tex_coord: Vec2::new(0.0, 1.0),
            color,
        });

        // top left -> top right -> bottom right
        self.push_triangle(idx + 0, idx + 1, idx + 2);
        // bottom right -> bottom left -> top left
        self.push_triangle(idx + 2, idx + 3, idx + 0);

        self.commit_primitive(None);
    }

    fn push_rect_filled(&mut self, coords: Rect, fill: Fill<R>) {
        let idx = self.vertices.len() as u32;

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
        self.push_vertex(Vertex {
            position: coords.top_left(),
            tex_coord: tex_coords
                .as_ref()
                .map(|tc| tc.top_left())
                .unwrap_or(Vec2::new(0.0, 0.0)),
            color: color.clone(),
        });
        // top right
        self.push_vertex(Vertex {
            position: coords.top_right(),
            tex_coord: tex_coords
                .as_ref()
                .map(|tc| tc.top_right())
                .unwrap_or(Vec2::new(1.0, 0.0)),
            color: color.clone(),
        });
        // bottom right
        self.push_vertex(Vertex {
            position: coords.bottom_right(),
            tex_coord: tex_coords
                .as_ref()
                .map(|tc| tc.bottom_right())
                .unwrap_or(Vec2::new(1.0, 1.0)),
            color: color.clone(),
        });
        // bottom left
        self.push_vertex(Vertex {
            position: coords.bottom_left(),
            tex_coord: tex_coords
                .as_ref()
                .map(|tc| tc.bottom_left())
                .unwrap_or(Vec2::new(0.0, 1.0)),
            color,
        });

        // top left -> top right -> bottom right
        self.push_triangle(idx + 0, idx + 1, idx + 2);
        // bottom right -> bottom left -> top left
        self.push_triangle(idx + 2, idx + 3, idx + 0);

        self.commit_primitive(texture);
    }

    fn push_rect_stroked(&mut self, coords: Rect, stroke: Stroke) {
        let top_left = coords.top_left();
        let top_right = coords.top_right();
        let bottom_right = coords.bottom_right();
        let bottom_left = coords.bottom_left();

        let width = stroke.width;
        let offset = width * 0.5;

        // horizontal lines:
        // extened to left and right by stroke width, shifted to top by half of
        // stroke width.
        self.push_line(LineShape::new(
            Vec2::new(top_left.x - width, top_left.y - offset),
            Vec2::new(top_right.x + width, top_right.y - offset),
            stroke.clone(),
        ));
        self.push_line(LineShape::new(
            Vec2::new(bottom_left.x - width, bottom_left.y + offset),
            Vec2::new(bottom_right.x + width, bottom_right.y + offset),
            stroke.clone(),
        ));

        // vertical lines:
        // shifted to right and left by half of stroke width
        self.push_line(LineShape::new(
            Vec2::new(top_right.x + offset, top_right.y),
            Vec2::new(bottom_right.x + offset, bottom_right.y),
            stroke.clone(),
        ));
        self.push_line(LineShape::new(
            Vec2::new(top_left.x - offset, top_left.y),
            Vec2::new(bottom_left.x - offset, bottom_left.y),
            stroke,
        ));

        self.commit_primitive(None);
    }

    pub fn push_rect(&mut self, rect: RectShape<R>) {
        if let Some(fill) = rect.fill {
            self.push_rect_filled(rect.coords.clone(), fill);
        }
        if let Some(stroke) = rect.stroke {
            self.push_rect_stroked(rect.coords, stroke);
        }
    }
}
