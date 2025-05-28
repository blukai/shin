use glam::Vec2;

use crate::{Fill, LineShape, Rect, RectShape, Stroke, Vertex, renderer::Renderer};

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

// TODO: do Primitive instead of DrawCommand?
#[derive(Debug)]
pub struct DrawCommand<R: Renderer> {
    pub start_index: u32,
    pub end_index: u32,
    pub tex_handle: Option<R::TextureHandle>,
}

#[derive(Debug)]
pub struct DrawBuffer<R: Renderer> {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pending_indices: usize,
    pub draw_commands: Vec<DrawCommand<R>>,
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

    fn commit_primitive(&mut self, tex_handle: Option<R::TextureHandle>) {
        if self.pending_indices == 0 {
            return;
        }
        self.draw_commands.push(DrawCommand {
            start_index: (self.indices.len() - self.pending_indices) as u32,
            end_index: self.indices.len() as u32,
            tex_handle,
        });
        self.pending_indices = 0;
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

        let (color, tex_handle, tex_coords) = if let Some(tex) = fill.texture {
            (fill.color, Some(tex.handle), Some(tex.coords))
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

        self.commit_primitive(tex_handle);
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

    pub fn push_rect(&mut self, shape: RectShape<R>) {
        if let Some(fill) = shape.fill {
            self.push_rect_filled(shape.coords.clone(), fill);
        }
        if let Some(stroke) = shape.stroke {
            self.push_rect_stroked(shape.coords, stroke);
        }
    }
}
