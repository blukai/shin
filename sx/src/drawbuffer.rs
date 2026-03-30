use std::ops::Range;

use mars::alloc;
use mars::array::GrowableArray;
use mars::sortedarray::SpillableSortedArrayMap;
use mars::string::FixedString;

use crate::{Rect, Rgba8, TextureHandle, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineId(pub u32);

impl Default for PipelineId {
    fn default() -> Self {
        Self(u32::MAX)
    }
}

// TODO: do the dynamic vertex attribute thing.
#[repr(C)]
#[derive(Debug)]
pub struct Vertex {
    /// screen pixel coordinates.
    /// 0, 0 is the top left corner of the screen.
    pub position: Vec2,
    pub color: Rgba8,
    /// normalized texture coordinates.
    /// 0, 0 is the top left corner of the texture.
    /// 1, 1 is the bottom right corner of the texture.
    pub uv: Vec2,
}

pub const NAME_MAX_LEN: usize = 32;
pub type Name = FixedString<NAME_MAX_LEN>;

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

pub struct MaterialDesc<'a> {
    pub pipeline: PipelineId,
    pub textures: &'a [(&'a str, TextureHandle)],
    pub uniforms: &'a [(&'a str, UniformValue)],
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Material {
    pub pipeline: PipelineId,
    pub textures: SpillableSortedArrayMap<Name, TextureHandle, 2, alloc::Global>,
    pub uniforms: SpillableSortedArrayMap<Name, UniformValue, 16, alloc::Global>,
}

impl Material {
    pub fn from_desc(desc: MaterialDesc<'_>) -> Self {
        let mut this = Self {
            pipeline: desc.pipeline,
            textures: SpillableSortedArrayMap::default(),
            uniforms: SpillableSortedArrayMap::default(),
        };
        for (name, handle) in desc.textures.iter() {
            this.textures
                .insert(Name::new_fixed().with_str(name), *handle);
        }
        for (name, value) in desc.uniforms.iter() {
            this.uniforms
                .insert(Name::new_fixed().with_str(name), *value);
        }
        this
    }

    pub fn set_texture(&mut self, name: &str, handle: TextureHandle) {
        self.textures
            .insert(Name::new_fixed().with_str(name), handle);
    }

    pub fn get_texture(&self, name: &str) -> Option<TextureHandle> {
        self.textures
            .get(&Name::new_fixed().with_str(name))
            .copied()
    }

    pub fn set_uniform(&mut self, name: &str, value: UniformValue) {
        self.uniforms
            .insert(Name::new_fixed().with_str(name), value);
    }

    pub fn get_uniform(&self, name: &str) -> Option<UniformValue> {
        self.uniforms
            .get(&Name::new_fixed().with_str(name))
            .copied()
    }
}

impl PartialEq<MaterialDesc<'_>> for Material {
    fn eq(&self, other: &MaterialDesc<'_>) -> bool {
        if self.pipeline != other.pipeline {
            return false;
        }

        if self.textures.0.len() != other.textures.len() {
            return false;
        }
        if self.uniforms.0.len() != other.uniforms.len() {
            return false;
        }

        for (name, value) in other.textures {
            match self.get_texture(name) {
                Some(v) if v == *value => {}
                _ => return false,
            }
        }
        for (name, value) in other.uniforms {
            match self.get_uniform(name) {
                Some(v) if &v == value => {}
                _ => return false,
            }
        }

        true
    }
}

#[derive(Debug)]
pub struct Quad<T> {
    pub ne: T,
    pub se: T,
    pub sw: T,
    pub nw: T,
}

impl Quad<Vec2> {
    pub fn from_rect(rect: Rect) -> Self {
        Self {
            ne: rect.northeast(),
            se: rect.southeast(),
            sw: rect.southwest(),
            nw: rect.northwest(),
        }
    }
}

impl<T> Quad<T> {
    pub fn splat(v: T) -> Self
    where
        T: Copy,
    {
        Self {
            ne: v,
            se: v,
            sw: v,
            nw: v,
        }
    }
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

#[derive(Debug)]
pub struct DrawCommand {
    pub index_range: Range<u32>,
    pub scissor: Option<Rect>,
    pub material: Material,
}

#[derive(Debug, Default)]
pub struct DrawData {
    pub vertices: GrowableArray<Vertex, alloc::Global>,
    pub indices: GrowableArray<u32, alloc::Global>,
    pub commands: GrowableArray<DrawCommand, alloc::Global>,
    pending_indices: u32,
    current_scissor: Option<Rect>,
    current_material: Material,
}

impl DrawData {
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();
        assert_eq!(self.pending_indices, 0);
        self.current_scissor = None;
        self.current_material = Material::default();
    }

    pub fn flush(&mut self) {
        if self.pending_indices == 0 {
            return;
        }

        let end_index = self.indices.len() as u32;
        let start_index = end_index - self.pending_indices;
        self.commands.push(DrawCommand {
            index_range: start_index..end_index,
            scissor: self.current_scissor,
            material: self.current_material.clone(),
        });

        self.pending_indices = 0;
    }

    pub fn set_scissor(&mut self, rect: Option<Rect>) {
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

    pub fn set_material_from_desc(&mut self, desc: MaterialDesc<'_>) {
        if self.current_material == desc {
            return;
        }
        self.flush();
        self.current_material = Material::from_desc(desc);
    }

    pub fn push_indices(&mut self, i0: u32, i1: u32, i2: u32) {
        self.indices.push(i0);
        self.indices.push(i1);
        self.indices.push(i2);
        self.pending_indices += 3;
    }

    // ----
    // 2_xcu

    // TODO: what if you could create some kind of material "sessions"?
    //   that might be cool, but could also super such for the same reasons scope guard on scissor
    //   guard sucked.

    fn push_vertex_xcu2(&mut self, position: Vec2, color: Rgba8, uv: Vec2) {
        self.vertices.push(Vertex {
            position,
            color,
            uv,
        });
    }

    fn push_quad_xcu2(&mut self, position: Quad<Vec2>, color: Quad<Rgba8>, uv: Quad<Vec2>) {
        let base = self.vertices.len() as u32;

        self.push_vertex_xcu2(position.nw, color.nw, uv.nw);
        self.push_vertex_xcu2(position.ne, color.ne, uv.ne);
        self.push_vertex_xcu2(position.se, color.se, uv.se);
        self.push_vertex_xcu2(position.sw, color.sw, uv.sw);

        // nw -> ne -> se (top left -> top right -> bottom right)
        self.push_indices(base + 0, base + 1, base + 2);
        // se -> sw -> ne (bottom right -> bottom left -> top left)
        self.push_indices(base + 2, base + 3, base + 0);
    }

    pub fn push_rect_xcu2_color(&mut self, pipeline: PipelineId, rect: Rect, color: Rgba8) {
        self.set_material_from_desc(MaterialDesc {
            pipeline,
            textures: &[],
            uniforms: &[],
        });

        self.push_quad_xcu2(
            Quad::from_rect(rect),
            Quad::splat(color),
            // TODO: shouldn't need this.
            Quad::splat(Vec2::ZERO),
        );
    }

    pub fn push_rect_xcu2_monochrome(
        &mut self,
        pipeline: PipelineId,
        rect: Rect,
        tint: Rgba8,
        uv: Rect,
        texture: TextureHandle,
    ) {
        self.set_material_from_desc(MaterialDesc {
            pipeline,
            textures: &[("tex", texture)],
            uniforms: &[],
        });

        self.push_quad_xcu2(
            Quad::from_rect(rect),
            Quad::splat(tint),
            Quad::from_rect(uv),
        );
    }

    pub fn push_rect_xcu2_sdf_stroked(
        &mut self,
        pipeline: PipelineId,
        rect: Rect,
        color: Rgba8,
        corner_radius: f32,
        stroke_width: f32,
        stroke_color: Rgba8,
        stroke_alignment: StrokeAlignment,
    ) {
        use UniformValue::*;
        self.set_material_from_desc(MaterialDesc {
            pipeline,
            textures: &[],
            uniforms: &[
                ("center", Float2(rect.center().to_array())),
                ("size", Float2(rect.size().to_array())),
                ("corner_radius", Float(corner_radius)),
                ("stroke_width", Float(stroke_width)),
                ("stroke_color", Float4(stroke_color.to_f32_array())),
                ("stroke_alignment", Int(stroke_alignment as i32)),
            ],
        });

        let rect = match stroke_alignment {
            StrokeAlignment::Inside => rect,
            // NOTE: in cases of outside/center outline rect needs to be scaled up.
            // this does not change size of the rect, no; but "reserves" space for the
            // outline.
            StrokeAlignment::Outside => rect.inflate(Vec2::splat(stroke_width)),
            StrokeAlignment::Center => rect.inflate(Vec2::splat(stroke_width * 0.5)),
        };

        self.push_quad_xcu2(
            Quad::from_rect(rect),
            Quad::splat(color),
            // TODO: shouldn't need this.
            Quad::splat(Vec2::ZERO),
        );
    }

    pub fn push_rect_xcu2_sdf(
        &mut self,
        pipeline: PipelineId,
        rect: Rect,
        color: Rgba8,
        corner_radius: f32,
    ) {
        self.push_rect_xcu2_sdf_stroked(
            pipeline,
            rect,
            color,
            corner_radius,
            0.0,
            Rgba8::TRANSPARENT,
            StrokeAlignment::default(),
        )
    }
}

// // ----
//
// // pipelines:
// //   - x_color
// //     uniforms: color
// //   - xu_monochrome
// //     uniforms: tint
// //     textures: texture
// //   - xu_polychrome
// //     uniforms: tint
// //     textures: texture
// //   - x_sdf_rect
// //     uniforms: center, size, corner_radii, stroke_width, stroke_color, stroke_align
//
// fn push_rect_x_color      (draw_data: &mut DrawData, x_color: PipelineHandle,
//   rect: Rect, color: Rgba8) {}
//
// fn push_rect_xu_monochrome(draw_data: &mut DrawData, xu_monochrome: PipelineHandle,
//   rect: Rect, tint: Rgba8, uv: Rect, texture: TextureHandle) {}
//
// fn push_rect_xu_polychrome(draw_data: &mut DrawData, xu_polychrome: PipelineHandle,
//   rect: Rect, tint: Rgba8, uv: Rect, texture: TextureHandle) {}
//
// fn push_rect_x_sdf        (draw_data: &mut DrawData, x_sdf_rect: PipelineHandle,
//   rect: Rect, color: Rgba8, corner_radii: CornerRadii) {}
//
// fn push_rect_x_sdf_stroked(draw_data: &mut DrawData, x_sdf_rect: PipelineHandle,
//   rect: Rect, color: Rgba8, corner_radii: CornerRadii, stroke_width: StrokeWidth, stroke_color: Rgba8, stroke_align: StrokeAlign) {}
//
// fn push_line_x_color      (draw_data: &mut DrawData, x_color: PipelineHandle,
//   p1: Vec2, p2: Vec2, width: f32, color: Rgba8) {}

//
//
//
// dead:
//
//
//
// pub fn push_line(&mut self, line_shape: LineShape) {
//     // NOTE: line's stroke may only be centered.
//     //   specifying outside/inside stroke alignment makes sense only for shapes that need an
//     //   outline (/ need to be stroked).
//     assert!(matches!(
//         line_shape.stroke.alignment,
//         StrokeAlignment::Center
//     ));
//
//     // NOTE: there's no sdf params for line.
//     self.draw_data.set_sdf_params(None);
//
//     // computes the vertex position offset away the from center caused by line width.
//     #[inline]
//     fn compute_line_width_offset(a: Vec2, b: Vec2, width: f32) -> Vec2 {
//         // direction defines how the line is oriented in space. it allows to know
//         // which way to move the vertices to create the desired width.
//         let dir = b - a;
//
//         // normalizing the direction vector converts it into a unit vector (length
//         // of 1). normalization ensures that the offset is proportional to the line
//         // width, regardless of the line's length.
//         let norm_dir = dir.normalize_or_zero();
//
//         // create a vector that points outward from the line. we want to move the
//         // vertices away from the center of the line, not along its length.
//         let perp = norm_dir.perp();
//
//         // to distribute the offset evenly on both sides of the line
//         let offset = perp * (width * 0.5);
//
//         offset
//     }
//
//     let [a, b] = line_shape.points;
//     let Stroke { width, color, .. } = line_shape.stroke;
//     let offset = compute_line_width_offset(a, b, width);
//     self.push_rect_filled(Rect::new(a + offset, b - offset), Fill::Color(color));
// }
