use std::{fmt, ops};

use mars::{
    alloc::{self, AllocError},
    cstring::CString,
    memory::Memory,
    string::String,
    vector::SpillableVector,
};

use crate::{TextureHandle, Vec2};

// NOTE: make_fixed_string_newtype is kind of this:
// https://doc.rust-lang.org/rust-by-example/generics/new_types.html.
macro_rules! make_fixed_string_newtype {
    ($vis:vis $name:ident($n:expr)) => {
        #[derive(Debug, Clone, PartialEq)]
        $vis struct $name(pub mars::string::FixedString<$n>);

        impl $name {
            pub fn from_str(s: &str) -> Self {
                Self(String::new_fixed().try_with_str(s).unwrap_or_else(|_| {
                    panic!(
                        "cannot create {type_name}, \"{s}\" is too long (got {len}, want <= {MAX_SHADER_DEFINE_LEN})",
                        type_name = stringify!($name),
                        len = s.len(),
                    );
                }))
            }

            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl ops::Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.0.as_str()
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0.as_str() == *other
            }
        }
    };
}

macro_rules! make_spillable_set {
    ($vis:vis $name:ident($item:ty, $n:expr, $a:path)) => {
        #[derive(Debug, Default, Clone, PartialEq)]
        $vis struct $name(pub SpillableVector<$item, $n, $a>);

        impl ops::Deref for $name {
            type Target = [$item];

            fn deref(&self) -> &Self::Target {
                self.0.as_slice()
            }
        }
    }
}

// ----
// defines

pub const MAX_SHADER_DEFINE_LEN: usize = 32;
pub const INITIAL_SHADER_DEFINES_CAP: usize = 16;

make_fixed_string_newtype!(pub ShaderDefine(MAX_SHADER_DEFINE_LEN));

make_spillable_set!(pub ShaderDefines(ShaderDefine, INITIAL_SHADER_DEFINES_CAP, alloc::Global));

impl ShaderDefines {
    pub fn set(&mut self, name: &str) {
        for it in self.0.iter_mut() {
            if it.0.as_str() == name {
                return;
            }
        }
        self.0.push(ShaderDefine::from_str(name));
    }

    pub fn and_set(mut self, name: &str) -> Self {
        self.set(name);
        self
    }
}

// ----
// uniforms

pub const MAX_SHADER_UNIFORM_NAME_LEN: usize = 32;
pub const INITIAL_SHADER_UNIFORMS_CAP: usize = 16;

make_fixed_string_newtype!(pub ShaderUniformName(MAX_SHADER_UNIFORM_NAME_LEN));

impl ShaderUniformName {
    pub fn try_to_c_string_in<M: Memory<u8>>(&self, mem: M) -> Result<CString<M>, AllocError> {
        self.0.try_to_c_string_in(mem)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShaderUniformValue {
    Float(f32),
    Vec2(Vec2),
    // TODO: Mat4 struct?
    Mat4([[f32; 4]; 4]),
    Texture2D(TextureHandle),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderUniform {
    pub name: ShaderUniformName,
    pub value: ShaderUniformValue,
}

make_spillable_set!(pub ShaderUniforms(ShaderUniform, INITIAL_SHADER_UNIFORMS_CAP, alloc::Global));

impl ShaderUniforms {
    pub fn set(&mut self, name: &str, value: ShaderUniformValue) {
        for it in self.0.iter_mut() {
            if it.name.as_str() == name {
                it.value = value;
                return;
            }
        }
        self.0.push(ShaderUniform {
            name: ShaderUniformName::from_str(name),
            value,
        });
    }

    pub fn and_set(mut self, name: &str, value: ShaderUniformValue) -> Self {
        self.set(name, value);
        self
    }

    pub fn remove(&mut self, name: &str) -> Option<ShaderUniformValue> {
        for i in 0..self.0.len() {
            let it = &self.0[i];
            if PartialEq::eq(it.name.as_str(), name) {
                return self.0.remove(i).map(|u| u.value);
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShaderUniformType {
    Float,
    Vec2,
    Mat4,
    Sampler2D,
}

// TODO: ShaderUniformDesc might need to either include info about which stage(s) uniform must be
// visible on or ShaderUniformDescs need to exist not at the shader level, but at shader stage
// level.
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderUniformDesc {
    pub name: ShaderUniformName,
    pub ty: ShaderUniformType,
}

make_spillable_set!(pub ShaderUniformDescs(ShaderUniformDesc, INITIAL_SHADER_UNIFORMS_CAP, alloc::Global));

impl ShaderUniformDescs {
    pub fn set(&mut self, name: &str, ty: ShaderUniformType) {
        for it in self.0.iter_mut() {
            if it.name == name {
                it.ty = ty;
                return;
            }
        }
        self.0.push(ShaderUniformDesc {
            name: ShaderUniformName::from_str(name),
            ty,
        });
    }

    pub fn and_set(mut self, name: &str, ty: ShaderUniformType) -> Self {
        self.set(name, ty);
        self
    }
}

// ----

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

#[derive(Debug)]
pub struct ShaderStageDesc {
    pub source: ShaderSource,
    pub defines: ShaderDefines,
}

// TODO: ShaderDesc might need to change to be able to accommodate compute?
#[derive(Debug)]
pub struct ShaderDesc {
    pub vertex_stage: ShaderStageDesc,
    pub fragment_stage: ShaderStageDesc,
    // TODO: uniforms are not per-stage, but for the whole pipeline, right?
    //   but obviously some can be visible/used only in vertex stage, some in fragment, etc.
    pub uniforms: ShaderUniformDescs,
}

// ----

// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// pub struct ShaderHandle {
//     id: u32,
// }
//
// impl NoHash for ShaderHandle {}
//
// #[derive(Debug)]
// pub enum ShaderCommandKind<Desc> {
//     Create { desc: Desc },
//     Delete,
// }
//
// #[derive(Debug)]
// pub struct ShaderCommand<Desc> {
//     pub handle: ShaderHandle,
//     pub kind: ShaderCommandKind<Desc>,
// }
//
// #[derive(Default)]
// pub struct ShaderService {
//     next_id: u32,
//
//     descs: NoHashMap<ShaderHandle, ShaderDesc>,
//     commands: Vec<ShaderCommand<()>>,
// }
//
// impl ShaderService {
//     pub fn create(&mut self, desc: ShaderDesc) -> ShaderHandle {
//         let handle = ShaderHandle { id: self.next_id };
//         self.next_id += 1;
//
//         log::debug!("ShaderService::create: ({handle:?}: {desc:?})");
//
//         self.descs.insert(handle, desc);
//         self.commands.push(ShaderCommand {
//             handle,
//             kind: ShaderCommandKind::Create { desc: () },
//         });
//         handle
//     }
//
//     pub fn delete(&mut self, handle: ShaderHandle) {
//         log::debug!("ShaderService::delete: ({handle:?})");
//
//         let desc = self.descs.remove(&handle);
//         assert!(desc.is_some());
//         self.commands.push(ShaderCommand {
//             handle,
//             kind: ShaderCommandKind::Delete,
//         });
//     }
//
//     pub fn drain_commands(&mut self) -> impl Iterator<Item = ShaderCommand<&ShaderDesc>> {
//         self.commands.drain(..).map(|cmd| {
//             let kind = match cmd.kind {
//                 ShaderCommandKind::Create { desc: _ } => ShaderCommandKind::Create {
//                     desc: self.descs.get(&cmd.handle).expect("invalid handle"),
//                 },
//                 ShaderCommandKind::Delete => ShaderCommandKind::Delete,
//             };
//             ShaderCommand {
//                 handle: cmd.handle,
//                 kind,
//             }
//         })
//     }
// }

// #[derive(Debug, Clone, Copy)]
// pub enum ShaderStage {
//     Vertex,
//     Fragment,
// }
