use std::fmt;

use mars::{
    alloc::{self, AllocError, Allocator},
    array::GrowableArray,
    handlearray::{ErasedHandle, Handle, HandleArray},
    sortedarray::SpillableSortedArrayMap,
    string::SpillableString,
};

#[derive(Clone)]
pub enum ShaderSourceKind {
    Static(&'static [u8]),
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
pub enum ShaderSourceDesc<Defines> {
    // #version number profile_opt
    // https://registry.khronos.org/OpenGL/specs/gl/GLSLangSpec.4.60.pdf
    Glsl {
        version: u16,
        profile: GlslProfile,
        defines: Defines,
    },
}

#[derive(Debug, Clone)]
pub struct ShaderModuleDesc<Defines> {
    pub source_kind: ShaderSourceKind,
    pub source_desc: ShaderSourceDesc<Defines>,
}

#[derive(Debug, Clone)]
pub struct PipelineDesc<Defines> {
    pub vertex_shader_module: ShaderModuleDesc<Defines>,
    pub fragment_shader_module: ShaderModuleDesc<Defines>,
    // TODO: binding descriptors or something.
}

pub type ShaderSourceDescRef<'a> = ShaderSourceDesc<&'a [(&'a str, &'a str)]>;
pub type ShaderModuleDescRef<'a> = ShaderModuleDesc<&'a [(&'a str, &'a str)]>;
pub type PipelineDescRef<'a> = PipelineDesc<&'a [(&'a str, &'a str)]>;

pub const MAX_DEFINE_NAME_LEN: usize = 32;
// NOTE: usually the value is empty, but otherwise it's perhaps a short number or bool, etc.
//   if the empty / short assumption failes - you allocate.
pub const MAX_DEFINE_VALUE_LEN: usize = 5;
pub const INITIAL_DEFINES_CAP: usize = 16;

type DefineName<A> = SpillableString<MAX_DEFINE_NAME_LEN, A>;
type DefineValue<A> = SpillableString<MAX_DEFINE_NAME_LEN, A>;
type Defines<A> = SpillableSortedArrayMap<DefineName<A>, DefineValue<A>, INITIAL_DEFINES_CAP, A>;

type ShaderSourceDescOwned<A> = ShaderSourceDesc<Defines<A>>;
type ShaderModuleDescOwned<A> = ShaderModuleDesc<Defines<A>>;
type PipelineDescOwned<A> = PipelineDesc<Defines<A>>;

fn pipeline_desc_from_ref_to_owned_in<A: Allocator + Copy>(
    rf: PipelineDescRef<'_>,
    alloc: A,
) -> Result<PipelineDescOwned<A>, AllocError> {
    fn shader_module_desc_ref_to_owned_in<A: Allocator + Copy>(
        rf: ShaderModuleDescRef<'_>,
        alloc: A,
    ) -> Result<ShaderModuleDescOwned<A>, AllocError> {
        Ok(ShaderModuleDesc {
            source_kind: rf.source_kind,
            source_desc: match rf.source_desc {
                ShaderSourceDesc::Glsl {
                    version,
                    profile,
                    defines,
                } => {
                    let mut owned_defines = SpillableSortedArrayMap::new_spillable_in(alloc);
                    for (name, value) in defines.iter() {
                        let name = DefineName::new_spillable_in(alloc).try_with_str(name)?;
                        let value = DefineValue::new_spillable_in(alloc).try_with_str(value)?;
                        owned_defines
                            .try_insert(name, value)
                            .map_err(|_| AllocError)?;
                    }
                    ShaderSourceDesc::Glsl {
                        version,
                        profile,
                        defines: owned_defines,
                    }
                }
            },
        })
    }
    Ok(PipelineDescOwned {
        vertex_shader_module: shader_module_desc_ref_to_owned_in(rf.vertex_shader_module, alloc)?,
        fragment_shader_module: shader_module_desc_ref_to_owned_in(
            rf.fragment_shader_module,
            alloc,
        )?,
    })
}

// NOTE: the underlying handle is erased because i don't want to have to parametrize pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineHandle(ErasedHandle);

#[derive(Debug)]
pub enum PipelineCommandKind<Desc> {
    Create { desc: Desc },
}

#[derive(Debug)]
pub struct PipelineCommand<Desc> {
    handle: PipelineHandle,
    kind: PipelineCommandKind<Desc>,
}

// TODO: maybe parametrize pipeline service with allocator.
#[derive(Default)]
pub struct PipelineService {
    descs: HandleArray<PipelineDescOwned<alloc::Global>, alloc::Global>,
    commands: GrowableArray<PipelineCommand<()>, alloc::Global>,
}

impl PipelineService {
    pub fn create(&mut self, desc_ref: PipelineDescRef<'_>) -> PipelineHandle {
        log::debug!("PipelineService::create: {desc_ref:?}");

        let desc_owned = pipeline_desc_from_ref_to_owned_in(desc_ref, alloc::Global).expect("oom");
        let handle = PipelineHandle(self.descs.push(desc_owned).to_erased());
        self.commands.push(PipelineCommand {
            handle,
            kind: PipelineCommandKind::Create { desc: () },
        });
        handle
    }

    pub fn drain_commands(
        &mut self,
    ) -> impl Iterator<Item = PipelineCommand<&PipelineDescOwned<alloc::Global>>> {
        self.commands.drain(..).map(|cmd| PipelineCommand {
            handle: cmd.handle,
            kind: match cmd.kind {
                PipelineCommandKind::Create { desc: _ } => PipelineCommandKind::Create {
                    desc: self.descs.get(Handle::from_erased(cmd.handle.0)),
                },
            },
        })
    }
}
