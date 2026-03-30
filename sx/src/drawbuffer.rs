use std::ops::Range;
use std::slice;

use mars::alloc::{self, Allocator};
use mars::array::GrowableArray;
use mars::sortedarray::SpillableSortedArrayMap;
use mars::string::FixedString;

use crate::{Rect, Rgba8, TextureHandle, Vec2};

pub const NAME_MAX_LEN: usize = 32;
pub const VERTEX_ATTRIBUTES_INITIAL_CAP: usize = 5;

// TODO: move cast slice somewhere more appropriate.

// TODO: draw data with attribute layout set at creation.
//   do only 2d xcu for now.
//   keep only indices, and 2d xcu vertex and quad.

// TODO: cast's A and B must be Copy.
//   > Copy represents values that can be cloned via memcpy and which lack destructors
//   ("plain old data").
//   - https://smallcultfollowing.com/babysteps/blog/2024/06/26/claim-followup-1/
//
// NOTE: this is stolen from bytemuck.
//
// PANICS:
//   - if attempting to cast between a zst and a non-zst
//   - if the total bytes can't be evenly divided by the target type's size
//   - if the target type's alignment exceeds the source pointer's alignment
#[inline]
pub fn cast_slice<A, B>(a: &[A]) -> &[B] {
    // TODO: might want to throw a cold hint onto this branch
    // handle zero-sized types (zsts)
    if size_of::<A>() == 0 || size_of::<B>() == 0 {
        assert!(
            size_of::<A>() == size_of::<B>(),
            "cannot cast between zst and non-zst types"
        );
        assert!(
            align_of::<B>() <= align_of::<A>(),
            "zst target alignment can't exceed source"
        );

        // for zsts, the length remains the same as no actual data is stored.
        return unsafe { slice::from_raw_parts(a.as_ptr().cast(), a.len()) };
    }

    // ensure total bytes are divisible by the size of B
    assert!(
        size_of_val(a) % size_of::<B>() == 0,
        "slice length is incompatible"
    );

    // check alignment compatibility
    assert!(
        a.as_ptr().align_offset(align_of::<B>()) == 0,
        "pointer is not aligned"
    );

    let new_len = size_of_val(a) / size_of::<B>();
    unsafe { slice::from_raw_parts(a.as_ptr() as *const B, new_len) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineId(pub u32);

impl Default for PipelineId {
    fn default() -> Self {
        Self(u32::MAX)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VertexAttributeSemantic {
    Position,
    Normal,
    Tangent,
    Uv(u8),
    Color(u8),
    Joints(u8),
    Weights(u8),
    Custom(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VertexAttributeFormat {
    Float32x2,
    Float32x3,
    Float32x4,
    Unorm8x4,
}

impl VertexAttributeFormat {
    pub fn components(&self) -> usize {
        match self {
            Self::Float32x2 => 2,
            Self::Float32x3 => 3,
            Self::Float32x4 => 4,
            Self::Unorm8x4 => 4,
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Self::Float32x2 => size_of::<f32>() * 2,
            Self::Float32x3 => size_of::<f32>() * 3,
            Self::Float32x4 => size_of::<f32>() * 4,
            Self::Unorm8x4 => size_of::<u8>() * 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VertexAttribute {
    // TODO: name.
    // pub name: FixedString<NAME_MAX_LEN>,
    pub semantic: VertexAttributeSemantic,
    pub format: VertexAttributeFormat,
}

impl VertexAttribute {
    pub const POSITION2: VertexAttribute = VertexAttribute {
        semantic: VertexAttributeSemantic::Position,
        format: VertexAttributeFormat::Float32x2,
    };
    pub const COLOR: VertexAttribute = VertexAttribute {
        semantic: VertexAttributeSemantic::Color(0),
        format: VertexAttributeFormat::Unorm8x4,
    };
    pub const UV2: VertexAttribute = VertexAttribute {
        semantic: VertexAttributeSemantic::Uv(0),
        format: VertexAttributeFormat::Float32x2,
    };
}

#[derive(Debug, Clone)]
pub enum VertexAttributeValues<A: Allocator> {
    Float32x2(GrowableArray<[f32; 2], A>),
    Float32x3(GrowableArray<[f32; 3], A>),
    Float32x4(GrowableArray<[f32; 4], A>),
    Unorm8x4(GrowableArray<[u8; 4], A>),
}

impl<A: Allocator> VertexAttributeValues<A> {
    pub fn len(&self) -> usize {
        match self {
            Self::Float32x2(values) => values.len(),
            Self::Float32x3(values) => values.len(),
            Self::Float32x4(values) => values.len(),
            Self::Unorm8x4(values) => values.len(),
        }
    }

    pub fn clear(&mut self) {
        match self {
            Self::Float32x2(values) => values.clear(),
            Self::Float32x3(values) => values.clear(),
            Self::Float32x4(values) => values.clear(),
            Self::Unorm8x4(values) => values.clear(),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Float32x2(values) => cast_slice(values),
            Self::Float32x3(values) => cast_slice(values),
            Self::Float32x4(values) => cast_slice(values),
            Self::Unorm8x4(values) => cast_slice(values),
        }
    }

    pub fn format(&self) -> VertexAttributeFormat {
        match self {
            Self::Float32x2(_) => VertexAttributeFormat::Float32x2,
            Self::Float32x3(_) => VertexAttributeFormat::Float32x3,
            Self::Float32x4(_) => VertexAttributeFormat::Float32x4,
            Self::Unorm8x4(_) => VertexAttributeFormat::Unorm8x4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UniformValue {
    Int(i32),
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
    pub textures:
        SpillableSortedArrayMap<FixedString<NAME_MAX_LEN>, TextureHandle, 2, alloc::Global>,
    pub uniforms:
        SpillableSortedArrayMap<FixedString<NAME_MAX_LEN>, UniformValue, 16, alloc::Global>,
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
                .insert(FixedString::new_fixed().with_str(name), *handle);
        }
        for (name, value) in desc.uniforms.iter() {
            this.uniforms
                .insert(FixedString::new_fixed().with_str(name), *value);
        }
        this
    }

    pub fn set_texture(&mut self, name: &str, handle: TextureHandle) {
        self.textures
            .insert(FixedString::new_fixed().with_str(name), handle);
    }

    pub fn get_texture(&self, name: &str) -> Option<TextureHandle> {
        self.textures
            .get(&FixedString::new_fixed().with_str(name))
            .copied()
    }

    pub fn set_uniform(&mut self, name: &str, value: UniformValue) {
        self.uniforms
            .insert(FixedString::new_fixed().with_str(name), value);
    }

    pub fn get_uniform(&self, name: &str) -> Option<UniformValue> {
        self.uniforms
            .get(&FixedString::new_fixed().with_str(name))
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
    pub attributes: SpillableSortedArrayMap<
        VertexAttribute,
        VertexAttributeValues<alloc::Global>,
        VERTEX_ATTRIBUTES_INITIAL_CAP,
        alloc::Global,
    >,
    pub indices: GrowableArray<u32, alloc::Global>,
    pub commands: GrowableArray<DrawCommand, alloc::Global>,
    pending_indices: u32,
    current_scissor: Option<Rect>,
    current_material: Material,
}

impl DrawData {
    pub fn clear(&mut self) {
        for (_, values) in self.attributes.0.iter_mut() {
            values.clear()
        }
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
        if !self.attributes.contains(&VertexAttribute::POSITION2) {
            self.attributes.insert(
                VertexAttribute::POSITION2,
                VertexAttributeValues::Float32x2(GrowableArray::default()),
            );
        }
        let Some(VertexAttributeValues::Float32x2(values)) =
            self.attributes.get_mut(&VertexAttribute::POSITION2)
        else {
            unreachable!();
        };
        values.push(position.to_array());

        if !self.attributes.contains(&VertexAttribute::COLOR) {
            self.attributes.insert(
                VertexAttribute::COLOR,
                VertexAttributeValues::Unorm8x4(GrowableArray::default()),
            );
        }
        let Some(VertexAttributeValues::Unorm8x4(values)) =
            self.attributes.get_mut(&VertexAttribute::COLOR)
        else {
            unreachable!();
        };
        values.push(color.to_array());

        if !self.attributes.contains(&VertexAttribute::UV2) {
            self.attributes.insert(
                VertexAttribute::UV2,
                VertexAttributeValues::Float32x2(GrowableArray::default()),
            );
        }
        let Some(VertexAttributeValues::Float32x2(values)) =
            self.attributes.get_mut(&VertexAttribute::UV2)
        else {
            unreachable!();
        };
        values.push(uv.to_array());
    }

    fn push_quad_xcu2(&mut self, position: Quad<Vec2>, color: Quad<Rgba8>, uv: Quad<Vec2>) {
        let base = if let Some(values) = self.attributes.get(&VertexAttribute::POSITION2) {
            values.len() as u32
        } else {
            0
        };

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
