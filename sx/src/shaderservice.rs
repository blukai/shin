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

#[derive(Debug, Clone)]
pub struct ShaderSource {
    pub kind: ShaderSourceKind,
    pub desc: ShaderSourceDesc,
}

#[derive(Debug)]
pub struct ShaderStage {
    pub desc: ShaderStageDesc,
}

#[derive(Debug)]
pub struct ShaderDesc {
    pub vertex_stage: ShaderStageDesc,
    pub fragment_stage: ShaderStageDesc,
    // TODO: uniforms are not per-stage, but for the whole pipeline, right?
    //   but obviously some can be visible/used only in vertex stage, some in fragment, etc.
    pub uniforms: ShaderUniformDescs,
}
