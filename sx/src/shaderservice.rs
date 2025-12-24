use std::fmt;

use mars::{
    alloc,
    sortedarray::{SpillableSortedArrayMap, SpillableSortedArraySet},
    string::FixedString,
};

use crate::TextureHandle;

// ----
// shader defines

pub const MAX_SHADER_DEFINE_LEN: usize = 32;
pub const INITIAL_SHADER_DEFINES_CAP: usize = 16;

pub type ShaderDefine = FixedString<MAX_SHADER_DEFINE_LEN>;
pub type ShaderDefines =
    SpillableSortedArraySet<ShaderDefine, INITIAL_SHADER_DEFINES_CAP, alloc::Global>;

// ----
// shader uniforms

pub const MAX_SHADER_UNIFORM_NAME_LEN: usize = 32;
pub const INITIAL_SHADER_UNIFORMS_CAP: usize = 16;

pub type ShaderUniformName = FixedString<MAX_SHADER_UNIFORM_NAME_LEN>;

#[derive(Debug, Clone, PartialEq)]
pub enum ShaderUniformValue {
    Int(i32),
    Float(f32),
    Vec2([f32; 2]),
    Vec4([f32; 4]),
    Mat4([[f32; 4]; 4]),
    Texture2D(TextureHandle),
}

pub type ShaderUniforms = SpillableSortedArrayMap<
    ShaderUniformName,
    ShaderUniformValue,
    INITIAL_SHADER_UNIFORMS_CAP,
    alloc::Global,
>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShaderUniformType {
    Int,
    Float,
    Vec2,
    Vec4,
    Mat4,
    Sampler2D,
}

pub type ShaderUniformDescs = SpillableSortedArrayMap<
    ShaderUniformName,
    ShaderUniformType,
    INITIAL_SHADER_UNIFORMS_CAP,
    alloc::Global,
>;

// ----
// shader source

#[derive(Clone)]
pub enum ShaderSourceKind {
    Static(&'static str),
    // TODO: FilePath.
}

impl fmt::Debug for ShaderSourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(..) => f.write_str("<static>"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GlslProfile {
    Core,
    Compatibility,
    Es,
}

#[derive(Debug, Clone)]
pub enum ShaderSourceDesc {
    // NOTE: glsl source is not expected to contain version string.
    //   #version number profile_opt
    //   https://registry.khronos.org/OpenGL/specs/gl/GLSLangSpec.4.60.pdf
    Glsl { version: u16, profile: GlslProfile },
}

#[derive(Debug, Clone)]
pub struct ShaderSource {
    pub kind: ShaderSourceKind,
    pub desc: ShaderSourceDesc,
}

// ----
// shader stage

#[derive(Debug, Clone, Copy)]
pub enum ShaderStageKind {
    Vertex,
    Fragment,
}

#[derive(Debug)]
pub struct ShaderStageDesc {
    pub source: ShaderSource,
    pub defines: ShaderDefines,
}

#[derive(Debug)]
pub struct ShaderStage {
    pub kind: ShaderStageKind,
    pub desc: ShaderStageDesc,
}

// TODO: most likely get rid of this? or maybe it should operate not on stage descs, but on stage
// handles / already created stages?
//
// TODO: ShaderDesc might need to change to be able to accommodate compute?
#[derive(Debug)]
pub struct ShaderDesc {
    pub vertex_stage: ShaderStageDesc,
    pub fragment_stage: ShaderStageDesc,
    // TODO: uniforms are not per-stage, but for the whole pipeline, right?
    //   but obviously some can be visible/used only in vertex stage, some in fragment, etc.
    pub uniforms: ShaderUniformDescs,
}

// TODO: shader service
//   must make it possible to attach shaders (fragment-only, or full pipeline thing) to draw buffer
//   shapes?
