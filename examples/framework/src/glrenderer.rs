use std::collections::HashMap;
use std::ffi::c_void;
use std::fmt::{self, Write as _};
use std::ptr::null;

use anyhow::{Context as _, anyhow};
use gl::wrap::Adapter;
use mars::alloc::{self, Allocator, TempAllocator};
use mars::arraymemory::FixedArrayMemory;
use mars::fxhash::FxBuildHasher;
use mars::scopeguard::ScopeGuard;
use mars::sortedarray::SpillableSortedArrayMap;
use mars::string::{FixedString, GrowableString, String};

// TODO: maybe ubo
// TODO: do i want to generate uniforms from shader desc?
// TODO: do i want to generate vertex input state from what? primitive attributes or something?

pub const PIPELINE_XCU2_COLOR: sx::PipelineId = sx::PipelineId(0);
pub const PIPELINE_XCU2_MONOCHROME: sx::PipelineId = sx::PipelineId(1);
pub const PIPELINE_XCU2_SDF_RECT: sx::PipelineId = sx::PipelineId(2);

const INITIAL_UNIFORMS_CAP: usize = 16;

#[derive(Clone)]
enum ShaderSourceKind {
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
enum GlslProfile {
    Core,
    Compatibility,
    Es,
}

#[derive(Debug, Clone, Copy)]
enum ShaderSourceDesc<'a> {
    // NOTE: glsl source is not expected to contain version string.
    //   #version number profile_opt
    //   https://registry.khronos.org/OpenGL/specs/gl/GLSLangSpec.4.60.pdf
    Glsl {
        version: u16,
        profile: GlslProfile,
        defines: &'a [(&'a str, &'a str)],
    },
}

#[derive(Debug, Clone)]
struct ShaderSource<'a> {
    kind: ShaderSourceKind,
    desc: ShaderSourceDesc<'a>,
}

#[derive(Debug)]
struct ShaderDesc<'a> {
    vertex_source: ShaderSource<'a>,
    fragment_source: ShaderSource<'a>,
    // TODO: uniforms are not per-stage, but for the whole pipeline, right?
    //   but obviously some can be visible/used only in vertex stage, some in fragment, etc.
    uniforms: &'a [&'a str],
    textures: &'a [&'a str],
}

#[cfg(target_family = "wasm")]
mod shader_consts {
    use super::*;
    pub const SHADER_VERSION: u16 = 300;
    pub const SHADER_PROFILE: sx::GlslProfile = sx::GlslProfile::Es;
}
#[cfg(not(target_family = "wasm"))]
mod shader_consts {
    use super::*;
    pub const SHADER_VERSION: u16 = 330;
    pub const SHADER_PROFILE: GlslProfile = GlslProfile::Core;
}
use shader_consts::*;

// NOTE: some kind of naming conventions for shader things
//   - `a_` for attributes
//   - `v_` for vertex-to-fragment outputs

const A_POSITION2_LOC: gl::GLuint = 0;
const A_COLOR_LOC: gl::GLuint = 1;
const A_UV2_LOC: gl::GLuint = 2;

const SHADER_XCU2_COLOR: &str = "
uniform mat4  projection_matrix;
uniform float scale_factor;

#if defined(SHADER_STAGE_VERTEX)
layout(location = 0) in vec2 a_position;
layout(location = 1) in vec4 a_color;
layout(location = 2) in vec2 a_uv;

out vec2 v_uv;
out vec4 v_color;

void main() {
    v_uv = a_uv;
    v_color = a_color / 255.0; // normalize 0..255 to 0.0..1.0
    gl_Position = projection_matrix * vec4(a_position, 0.0, 1.0);
}
#endif

#if defined(SHADER_STAGE_FRAGMENT)
in vec2 v_uv;
in vec4 v_color;

out vec4 FragColor;

void main() {
    FragColor = v_color;
}
#endif
";

const SHADER_XCU2_MONOCHROME: &str = "
uniform mat4  projection_matrix;
uniform float scale_factor;

#if defined(SHADER_STAGE_VERTEX)
layout(location = 0) in vec2 a_position;
layout(location = 1) in vec4 a_color;
layout(location = 2) in vec2 a_uv;

out vec2 v_uv;
out vec4 v_color;

void main() {
    v_uv = a_uv;
    v_color = a_color / 255.0; // normalize 0..255 to 0.0..1.0
    gl_Position = projection_matrix * vec4(a_position, 0.0, 1.0);
}
#endif

#if defined(SHADER_STAGE_FRAGMENT)
in vec2 v_uv;
in vec4 v_color;

uniform sampler2D tex;

out vec4 FragColor;

void main() {
    FragColor = v_color;
    FragColor.a *= texture(tex, v_uv).r;
}
#endif
";

const SHADER_XCU2_SDF_RECT: &str = "
uniform mat4  projection_matrix;
uniform float scale_factor;

#if defined(SHADER_STAGE_VERTEX)
layout(location = 0) in vec2 a_position;
layout(location = 1) in vec4 a_color;
layout(location = 2) in vec2 a_uv;

out vec2 v_uv;
out vec4 v_color;

void main() {
    v_uv = a_uv;
    v_color = a_color / 255.0; // normalize 0..255 to 0.0..1.0
    gl_Position = projection_matrix * vec4(a_position, 0.0, 1.0);
}
#endif

#if defined(SHADER_STAGE_FRAGMENT)
in vec2 v_uv;
in vec4 v_color;

uniform vec2  center;
uniform vec2  size;
uniform float corner_radius;
uniform float stroke_width;
uniform vec4  stroke_color;
uniform int   stroke_alignment; // -1 inside, 0 center, 1 outside

// https://iquilezles.org/articles/distfunctions2d/
float sd_rounded_box(vec2 p, vec2 b, vec4 r) {
    r.xy = (p.x > 0.0) ? r.xy : r.zw;
    r.x  = (p.y > 0.0) ? r.x  : r.y;
    vec2 q = abs(p) - b + r.x;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r.x;
}

// https://en.wikipedia.org/wiki/Alpha_compositing
// https://www.w3.org/TR/compositing-1/#whatiscompositing
vec4 composite_rgba(vec4 bg, vec4 fg) {
    vec3 cs = fg.rgb;
    float as = fg.a;
    vec3 cb = bg.rgb;
    float ab = bg.a;
    vec3 co = cs * as + cb * ab * (1.0 - as);
    float ao = as + ab * (1.0 - as);
    return vec4(co / ao, ao);
}

vec4 composite_rgba_with_coverage(vec4 bg, float bg_cov, vec4 fg, float fg_cov) {
    // effective alphas
    bg.a *= bg_cov;
    fg.a *= fg_cov;
    return composite_rgba(bg, fg);
}

// returns (inner, outer) stroke parts where outer + inner = width.
//   (16.0, -1.0) -> fully inner  ( 0.0, 16.0)
//   (16.0,  0.0) -> centered     ( 8.0,  8.0)
//   (16.0, +1.0) -> fully outer  (16.0,  0.0)
vec2 split_stroke(float width, int alignment) {
    float inner = width * 0.5 * (1.0 - float(alignment));
    float outer = width * 0.5 * (1.0 + float(alignment));
    return vec2(inner, outer);
}

vec4 sdf_rect(
    vec2 frag_pos,
    vec4 frag_color,
    vec2 center,
    vec2 size,
    float corner_radius,
    float stroke_width,
    int stroke_alignment,
    vec4 stroke_color
) {
    vec2 stroke_split = split_stroke(stroke_width, stroke_alignment);
    float stroke_inner = stroke_split.x;
    float stroke_outer = stroke_split.y;

    vec2 p = frag_pos - center;
    vec2 b = size * 0.5 + stroke_outer;
    // TODO: maybe need to select specific corner's radius; this wont work with radii.
    float r_zero_mask = float(int(corner_radius > 0.0));
    float r = (corner_radius + stroke_outer) * r_zero_mask;

    float stroke_dist_outer = sd_rounded_box(p, b, vec4(r));
    // stroke_dist_outer(-32.5), stroke_outer(0.5) -> -32.0
    float fill_dist = stroke_dist_outer + stroke_outer;
    // fill_dist(-32.0), stroke_inner(0.5) -> -31.5
    float stroke_dist_inner = fill_dist + stroke_inner;

    // TODO: maybe better aa?
    //   but don't use fwidth, it sucks.
    //   also see https://mini.gmshaders.com/p/antialiasing
    float aa = 0.5;
    float fill_cov = 1.0 - smoothstep(-aa, aa, fill_dist);
    float stroke_cov_inner = 1.0 - smoothstep(-aa, aa, stroke_dist_inner);
    float stroke_cov_outer = 1.0 - smoothstep(-aa, aa, stroke_dist_outer);
    float stroke_cov = stroke_cov_outer - stroke_cov_inner;

    return composite_rgba_with_coverage(frag_color, fill_cov, stroke_color, stroke_cov);
}

out vec4 FragColor;

void main() {
    FragColor = sdf_rect(
        gl_FragCoord.xy,
        v_color,
        center * scale_factor,
        size * scale_factor,
        corner_radius * scale_factor,
        stroke_width * scale_factor,
        stroke_alignment,
        stroke_color
    );
}
#endif
";

fn prefix_shader_source<'a, A: Allocator>(
    source: &ShaderSource,
    alloc: A,
) -> Result<GrowableString<A>, fmt::Error> {
    let ShaderSource {
        kind: ShaderSourceKind::Static(code),
        desc:
            ShaderSourceDesc::Glsl {
                version,
                profile,
                defines,
            },
    } = source;

    let mut ret = String::new_growable_in(alloc);

    match (version, profile) {
        (version, GlslProfile::Core) => {
            ret.write_fmt(format_args!("#version {version} core\n"))?;
        }
        (_version, GlslProfile::Compatibility) => {
            unimplemented!()
        }
        (version, GlslProfile::Es) => {
            ret.write_fmt(format_args!("#version {version} es\n"))?;
            // NOTE: type can only be float or int.
            //   see https://wikis.khronos.org/opengl/Type_Qualifier_(GLSL)#Precision_qualifiers
            ret.write_fmt(format_args!("precision highp float;\n"))?;
            ret.write_fmt(format_args!("precision highp int;\n"))?;
        }
    }

    for (name, value) in *defines {
        ret.write_fmt(format_args!("#define {name} {value}\n"))?;
    }

    ret.push_str(code);
    Ok(ret)
}

unsafe fn create_shader(
    gl_api: &gl::wrap::Api,
    src: &str,
    r#type: gl::GLenum,
) -> anyhow::Result<gl::wrap::Shader> {
    unsafe {
        let shader = gl_api
            .create_shader(r#type)
            .context("could not create shader")?;
        gl_api.shader_source(shader, src);
        gl_api.compile_shader(shader);

        let compile_status = gl_api.get_shader_parameter(shader, gl::COMPILE_STATUS);
        if compile_status == gl::FALSE as gl::GLint {
            let info_log = gl_api.get_shader_info_log(shader);
            Err(anyhow!("could not create shader: {info_log}"))
        } else {
            Ok(shader)
        }
    }
}

unsafe fn create_program(
    gl_api: &gl::wrap::Api,
    vert_src: &str,
    frag_src: &str,
) -> anyhow::Result<gl::wrap::Program> {
    unsafe {
        let vert_shader = create_shader(gl_api, vert_src, gl::VERTEX_SHADER)?;
        let frag_shader = create_shader(gl_api, frag_src, gl::FRAGMENT_SHADER)?;

        let program = gl_api
            .create_program()
            .context("could not create program")?;

        gl_api.attach_shader(program, vert_shader);
        gl_api.attach_shader(program, frag_shader);

        gl_api.link_program(program);

        gl_api.detach_shader(program, vert_shader);
        gl_api.detach_shader(program, frag_shader);

        gl_api.delete_shader(vert_shader);
        gl_api.delete_shader(frag_shader);

        let link_status = gl_api.get_program_parameter(program, gl::LINK_STATUS);
        if link_status == gl::FALSE as gl::GLint {
            let info_log = gl_api.get_program_info_log(program);
            Err(anyhow!("could not create program: {info_log}"))
        } else {
            Ok(program)
        }
    }
}

struct Shader {
    gl_handle: gl::wrap::Program,
    uniform_locations: SpillableSortedArrayMap<
        FixedString<{ sx::NAME_MAX_LEN }>,
        gl::wrap::UniformLocation,
        INITIAL_UNIFORMS_CAP,
        alloc::Global,
    >,
    texture_units:
        SpillableSortedArrayMap<FixedString<{ sx::NAME_MAX_LEN }>, gl::GLenum, 2, alloc::Global>,
}

impl Shader {
    fn new(
        desc: ShaderDesc<'_>,
        gl_api: &gl::wrap::Api,
        temp: &TempAllocator<'_>,
    ) -> anyhow::Result<Self> {
        let program = unsafe {
            let _guard = temp.checkpoint();
            create_program(
                gl_api,
                &prefix_shader_source(&desc.vertex_source, temp)
                    .context("could not prefix vertex shader")?,
                &prefix_shader_source(&desc.fragment_source, temp)
                    .context("could not prefix fragment shader")?,
            )
            .context("could not create program")?
        };

        unsafe { gl_api.use_program(Some(program)) };

        let mut uniform_locations = SpillableSortedArrayMap::default();
        for name in desc.uniforms {
            let name = FixedString::new_fixed().with_str(name);
            let Some(location) = ({
                let _guard = temp.checkpoint();
                let cname =
                    name.to_c_string_in(FixedArrayMemory::<_, { sx::NAME_MAX_LEN + 1 }>::default());
                unsafe { gl_api.get_uniform_location(program, cname.as_c_str()) }
            }) else {
                continue;
            };
            uniform_locations.insert(name, location);
        }

        let mut texture_units = SpillableSortedArrayMap::default();
        for (i, name) in desc.textures.iter().enumerate() {
            let name = FixedString::new_fixed().with_str(name);
            let Some(location) = ({
                let _guard = temp.checkpoint();
                let cname =
                    name.to_c_string_in(FixedArrayMemory::<_, { sx::NAME_MAX_LEN + 1 }>::default());
                unsafe { gl_api.get_uniform_location(program, cname.as_c_str()) }
            }) else {
                continue;
            };

            let unit = match i {
                0 => gl::TEXTURE0,
                1 => gl::TEXTURE1,
                other => return Err(anyhow!("unhandled texture unit ({other})")),
            };
            texture_units.insert(name, unit);

            // NOTE: texture needs to be to assigned to texture unit only once, right?
            //   learnopengl.com said so.
            //   @Unverified
            unsafe { gl_api.uniform_1i(location, i as gl::GLint) };
        }

        Ok(Shader {
            gl_handle: program,
            uniform_locations,
            texture_units,
        })
    }
}

fn create_shader_xcu2_color(
    gl_api: &gl::wrap::Api,
    temp: &TempAllocator<'_>,
) -> anyhow::Result<Shader> {
    let desc = ShaderDesc {
        vertex_source: ShaderSource {
            kind: ShaderSourceKind::Static(SHADER_XCU2_COLOR),
            desc: ShaderSourceDesc::Glsl {
                version: SHADER_VERSION,
                profile: SHADER_PROFILE,
                defines: &[("SHADER_STAGE_VERTEX", "")],
            },
        },
        fragment_source: ShaderSource {
            kind: ShaderSourceKind::Static(SHADER_XCU2_COLOR),
            desc: ShaderSourceDesc::Glsl {
                version: SHADER_VERSION,
                profile: SHADER_PROFILE,
                defines: &[("SHADER_STAGE_FRAGMENT", "")],
            },
        },
        uniforms: &["projection_matrix", "scale_factor"],
        textures: &[],
    };
    Shader::new(desc, gl_api, temp)
}

fn create_shader_xcu2_monochrome(
    gl_api: &gl::wrap::Api,
    temp: &TempAllocator<'_>,
) -> anyhow::Result<Shader> {
    let desc = ShaderDesc {
        vertex_source: ShaderSource {
            kind: ShaderSourceKind::Static(SHADER_XCU2_MONOCHROME),
            desc: ShaderSourceDesc::Glsl {
                version: SHADER_VERSION,
                profile: SHADER_PROFILE,
                defines: &[("SHADER_STAGE_VERTEX", "")],
            },
        },
        fragment_source: ShaderSource {
            kind: ShaderSourceKind::Static(SHADER_XCU2_MONOCHROME),
            desc: ShaderSourceDesc::Glsl {
                version: SHADER_VERSION,
                profile: SHADER_PROFILE,
                defines: &[("SHADER_STAGE_FRAGMENT", "")],
            },
        },
        uniforms: &["projection_matrix", "scale_factor"],
        textures: &["tex"],
    };
    Shader::new(desc, gl_api, temp)
}

fn create_shader_xcu2_sdf_rect(
    gl_api: &gl::wrap::Api,
    temp: &TempAllocator<'_>,
) -> anyhow::Result<Shader> {
    let desc = ShaderDesc {
        vertex_source: ShaderSource {
            kind: ShaderSourceKind::Static(SHADER_XCU2_SDF_RECT),
            desc: ShaderSourceDesc::Glsl {
                version: SHADER_VERSION,
                profile: SHADER_PROFILE,
                defines: &[("SHADER_STAGE_VERTEX", "")],
            },
        },
        fragment_source: ShaderSource {
            kind: ShaderSourceKind::Static(SHADER_XCU2_SDF_RECT),
            desc: ShaderSourceDesc::Glsl {
                version: SHADER_VERSION,
                profile: SHADER_PROFILE,
                defines: &[("SHADER_STAGE_FRAGMENT", "")],
            },
        },
        uniforms: &[
            "projection_matrix",
            "scale_factor",
            "center",
            "size",
            "corner_radius",
            "stroke_width",
            "stroke_color",
            "stroke_alignment",
        ],
        textures: &[],
    };
    Shader::new(desc, gl_api, temp)
}

struct TextureFormatDesc {
    internal_format: gl::GLint,
    format: gl::GLenum,
    ty: gl::GLenum,
    // like https://docs.vulkan.org/spec/latest/chapters/formats.html#texel-block-size
    block_size: gl::GLint,
}

fn describe_texture_format(format: sx::TextureFormat) -> TextureFormatDesc {
    match format {
        sx::TextureFormat::Rgba8Unorm => TextureFormatDesc {
            internal_format: gl::RGBA8 as _,
            format: gl::RGBA,
            ty: gl::UNSIGNED_BYTE,
            block_size: 4,
        },
        sx::TextureFormat::R8Unorm => TextureFormatDesc {
            internal_format: gl::R8 as _,
            format: gl::RED,
            ty: gl::UNSIGNED_BYTE,
            block_size: 1,
        },
    }
}

struct Texture {
    gl_handle: gl::wrap::Texture,
    format: sx::TextureFormat,
}

unsafe fn create_default_white_texture(gl_api: &gl::wrap::Api) -> anyhow::Result<Texture> {
    let format = sx::TextureFormat::R8Unorm;
    let format_desc = describe_texture_format(format);
    unsafe {
        let gl_handle = gl_api
            .create_texture()
            .context("could not create texture")?;
        gl_api.bind_texture(gl::TEXTURE_2D, Some(gl_handle));
        gl_api.tex_image_2d(
            gl::TEXTURE_2D,
            0,
            format_desc.internal_format,
            1,
            1,
            0,
            format_desc.format,
            format_desc.ty,
            {
                assert_eq!(format_desc.block_size, 1);
                [255_u8; 1].as_ptr().cast()
            },
        );
        Ok(Texture { gl_handle, format })
    }
}

struct Framebuffer {
    width: u32,
    height: u32,
    color_renderbuffer: gl::wrap::Renderbuffer,
    depth_renderbuffer: gl::wrap::Renderbuffer,
    framebuffer: gl::wrap::Framebuffer,
}

fn create_framebuffer(
    gl_api: &gl::wrap::Api,
    width: u32,
    height: u32,
) -> anyhow::Result<Framebuffer> {
    // TODO: scopeguard to cleanup created resources if something fails
    //   via checkpoint-based gl allocator

    unsafe {
        let color_renderbuffer = gl_api
            .create_renderbuffer()
            .context("could not create renderbuffer")?;
        gl_api.bind_renderbuffer(gl::RENDERBUFFER, Some(color_renderbuffer));
        gl_api.renderbuffer_storage(
            gl::RENDERBUFFER,
            gl::RGBA8,
            width as gl::GLint,
            height as gl::GLint,
        );
        gl_api.bind_renderbuffer(gl::RENDERBUFFER, None);

        let depth_renderbuffer = gl_api
            .create_renderbuffer()
            .context("could not create renderbuffer")?;
        gl_api.bind_renderbuffer(gl::RENDERBUFFER, Some(depth_renderbuffer));
        gl_api.renderbuffer_storage(
            gl::RENDERBUFFER,
            gl::DEPTH_COMPONENT,
            width as gl::GLint,
            height as gl::GLint,
        );
        gl_api.bind_renderbuffer(gl::RENDERBUFFER, None);

        let framebuffer = gl_api
            .create_framebuffer()
            .context("could not create framebuffer")?;
        gl_api.bind_framebuffer(gl::FRAMEBUFFER, Some(framebuffer));
        gl_api.framebuffer_renderbuffer(
            gl::FRAMEBUFFER,
            gl::COLOR_ATTACHMENT0,
            gl::RENDERBUFFER,
            Some(color_renderbuffer),
        );
        gl_api.framebuffer_renderbuffer(
            gl::FRAMEBUFFER,
            gl::DEPTH_ATTACHMENT,
            gl::RENDERBUFFER,
            Some(depth_renderbuffer),
        );
        let framebuffer_status = gl_api.check_framebuffer_status(gl::FRAMEBUFFER);
        if framebuffer_status != gl::FRAMEBUFFER_COMPLETE {
            return Err(anyhow!("framebuffer error: {framebuffer_status}"));
        }
        gl_api.bind_framebuffer(gl::FRAMEBUFFER, None);

        Ok(Framebuffer {
            width,
            height,
            color_renderbuffer,
            depth_renderbuffer,
            framebuffer,
        })
    }
}

fn delete_framebuffer(framebuffer: Framebuffer, gl_api: &gl::wrap::Api) {
    unsafe {
        gl_api.delete_renderbuffer(framebuffer.color_renderbuffer);
        gl_api.delete_renderbuffer(framebuffer.depth_renderbuffer);
        gl_api.delete_framebuffer(framebuffer.framebuffer);
    }
}

fn compute_orthographic_projection_matrix(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
) -> [[f32; 4]; 4] {
    let a = 2.0 / (right - left);
    let b = 2.0 / (top - bottom);
    let c = -2.0 / (far - near);
    let tx = -(right + left) / (right - left);
    let ty = -(top + bottom) / (top - bottom);
    let tz = -(far + near) / (far - near);
    [
        [a, 0.0, 0.0, 0.0],
        [0.0, b, 0.0, 0.0],
        [0.0, 0.0, c, 0.0],
        [tx, ty, tz, 1.0],
    ]
}

pub struct GlRenderer {
    framebuffer: Option<Framebuffer>,

    vbo_positions: gl::wrap::Buffer,
    vbo_colors: gl::wrap::Buffer,
    vbo_uvs: gl::wrap::Buffer,
    ebo: gl::wrap::Buffer,
    vao: gl::wrap::VertexArray,

    // TODO: make shaders into pipelines.
    shaders: HashMap<sx::PipelineId, Shader, FxBuildHasher>,
    default_white_texture: Texture,
    textures: HashMap<sx::TextureHandle, Texture, FxBuildHasher>,
}

impl GlRenderer {
    pub fn new(gl_api: &gl::wrap::Api, temp: &TempAllocator<'_>) -> anyhow::Result<Self> {
        // TODO: scopeguard to cleanup created resources if something fails
        //   via checkpoint-based gl allocator

        unsafe {
            let vbo_positions = gl_api.create_buffer().context("could not create vbo")?;
            let vbo_colors = gl_api.create_buffer().context("could not create vbo")?;
            let vbo_uvs = gl_api.create_buffer().context("could not create vbo")?;
            let ebo = gl_api.create_buffer().context("could not create ebo")?;
            let vao = {
                let vao = gl_api
                    .create_vertex_array()
                    .context("could not create vao")?;
                gl_api.bind_vertex_array(Some(vao));

                gl_api.bind_buffer(gl::ARRAY_BUFFER, Some(vbo_positions));
                gl_api.vertex_attrib_pointer(A_POSITION2_LOC, 2, gl::FLOAT, gl::FALSE, 0, null());
                gl_api.enable_vertex_attrib_array(A_POSITION2_LOC);

                gl_api.bind_buffer(gl::ARRAY_BUFFER, Some(vbo_colors));
                gl_api.vertex_attrib_pointer(
                    A_COLOR_LOC,
                    4,
                    gl::UNSIGNED_BYTE,
                    gl::FALSE,
                    0,
                    null(),
                );
                gl_api.enable_vertex_attrib_array(A_COLOR_LOC);

                gl_api.bind_buffer(gl::ARRAY_BUFFER, Some(vbo_uvs));
                gl_api.vertex_attrib_pointer(A_UV2_LOC, 2, gl::FLOAT, gl::FALSE, 0, null());
                gl_api.enable_vertex_attrib_array(A_UV2_LOC);

                vao
            };

            let mut shaders = HashMap::default();
            shaders.insert(PIPELINE_XCU2_COLOR, create_shader_xcu2_color(gl_api, temp)?);
            shaders.insert(
                PIPELINE_XCU2_MONOCHROME,
                create_shader_xcu2_monochrome(gl_api, temp)?,
            );
            shaders.insert(
                PIPELINE_XCU2_SDF_RECT,
                create_shader_xcu2_sdf_rect(gl_api, temp)?,
            );

            Ok(Self {
                framebuffer: None,

                vbo_positions,
                vbo_colors,
                vbo_uvs,
                ebo,
                vao,

                shaders,

                default_white_texture: create_default_white_texture(gl_api)
                    .context("could not create default white tex")?,
                textures: HashMap::default(),
            })
        }
    }

    // TODO: figure out how to invoke this xd.
    pub fn deinit(mut self, gl_api: &gl::wrap::Api) {
        unsafe {
            if let Some(framebuffer) = self.framebuffer.take() {
                delete_framebuffer(framebuffer, gl_api);
            }

            gl_api.delete_buffer(self.vbo_positions);
            gl_api.delete_buffer(self.vbo_colors);
            gl_api.delete_buffer(self.vbo_uvs);
            gl_api.delete_buffer(self.ebo);
            gl_api.delete_buffer(self.vao);

            for (_, shader) in self.shaders.iter() {
                gl_api.delete_program(shader.gl_handle);
            }

            gl_api.delete_texture(self.default_white_texture.gl_handle);
            for (_, texture) in self.textures.iter() {
                gl_api.delete_texture(texture.gl_handle);
            }
        }
    }

    pub fn handle_texture_commands<'a>(
        &mut self,
        texture_commands: impl Iterator<Item = sx::TextureCommand<&'a sx::TextureDesc, &'a [u8]>>,
        gl_api: &gl::wrap::Api,
    ) -> anyhow::Result<()> {
        for command in texture_commands {
            match command.kind {
                sx::TextureCommandKind::Create { desc } => {
                    assert!(!self.textures.contains_key(&command.handle));
                    let format_desc = describe_texture_format(desc.format);
                    let texture = unsafe {
                        let texture = gl_api
                            .create_texture()
                            .context("could not create texture")?;
                        gl_api.bind_texture(gl::TEXTURE_2D, Some(texture));

                        // NOTE: it seems like these parameters are getting stored to a texture
                        // that is currently bound.
                        //   people on the internet are saying this, but i coudn't find a
                        //   definitive proof really.
                        //
                        //   > glTexParameter specifies the texture parameters for the active
                        //   texture unit, specified by calling glActiveTexture.
                        //   - https://registry.khronos.org/OpenGL-Refpages/gl4/html/glTexParameter.xhtml
                        //
                        //   but i do not call glActiveTexture here, and it works. very confusing.

                        // NOTE: this fixes tilting when rendering bitmaps. see
                        // https://stackoverflow.com/questions/15983607/opengl-texture-tilted.
                        gl_api.pixel_storei(gl::UNPACK_ALIGNMENT, format_desc.block_size);

                        // NOTE: without those params you can't see shit in this mist
                        //
                        // NOTE: to deal with min and mag filters, etc. - you might want to
                        // consider introducing SamplerDescriptor and TextureViewDescriptor
                        gl_api.tex_parameteri(
                            gl::TEXTURE_2D,
                            gl::TEXTURE_MIN_FILTER,
                            gl::NEAREST as _,
                        );
                        gl_api.tex_parameteri(
                            gl::TEXTURE_2D,
                            gl::TEXTURE_MAG_FILTER,
                            gl::NEAREST as _,
                        );

                        gl_api.tex_image_2d(
                            gl::TEXTURE_2D,
                            0,
                            format_desc.internal_format,
                            desc.w as gl::GLint,
                            desc.h as gl::GLint,
                            0,
                            format_desc.format,
                            format_desc.ty,
                            null(),
                        );

                        texture
                    };
                    self.textures.insert(
                        command.handle,
                        Texture {
                            gl_handle: texture,
                            format: desc.format,
                        },
                    );
                }
                sx::TextureCommandKind::Upload { region, buf } => {
                    let texture = self
                        .textures
                        .get(&command.handle)
                        .expect("invalud texture handle");
                    let format_desc = describe_texture_format(texture.format);
                    unsafe {
                        gl_api.bind_texture(gl::TEXTURE_2D, Some(texture.gl_handle));
                        gl_api.tex_sub_image_2d(
                            gl::TEXTURE_2D,
                            0,
                            region.x as gl::GLint,
                            region.y as gl::GLint,
                            region.w as gl::GLsizei,
                            region.h as gl::GLsizei,
                            format_desc.format,
                            format_desc.ty,
                            buf.as_ptr().cast(),
                        );
                    }
                }
                sx::TextureCommandKind::Delete => {
                    let texture = self
                        .textures
                        .remove(&command.handle)
                        .unwrap_or_else(|| panic!("invalid handle: {:?}", &command.handle));
                    unsafe { gl_api.delete_texture(texture.gl_handle) };
                }
            }
        }
        Ok(())
    }

    pub fn render(
        &mut self,
        logical_size: sx::Vec2,
        scale_factor: f32,
        draw_data: &sx::DrawData,
        gl_api: &gl::wrap::Api,
        temp: &TempAllocator<'_>,
    ) -> anyhow::Result<()> {
        let physical_size = logical_size * scale_factor;
        // NOTE: this is opengl-specific matrix. y is up.
        //   glBlitFramebuffer flips whole thing.
        //   this way is easier because there's no need to micromanage each uniform value, etc.
        let projection_matrix = compute_orthographic_projection_matrix(
            0.0,
            logical_size.x,
            0.0,
            logical_size.y,
            -1.0,
            1.0,
        );

        if let Some(framebuffer) =
            self.framebuffer
                .take_if(|Framebuffer { width, height, .. }| {
                    *width != physical_size.x as u32 || *height != physical_size.y as u32
                })
        {
            delete_framebuffer(framebuffer, gl_api);
        }
        if self.framebuffer.is_none() {
            self.framebuffer = Some(create_framebuffer(
                gl_api,
                physical_size.x as u32,
                physical_size.y as u32,
            )?);
        }
        let Some(framebuffer) = self.framebuffer.as_ref() else {
            unreachable!();
        };

        let sx::DrawData {
            attributes,
            indices,
            commands,
            ..
        } = draw_data;

        unsafe {
            gl_api.bind_framebuffer(gl::FRAMEBUFFER, Some(framebuffer.framebuffer));

            gl_api.clear_color(0.0, 0.0, 0.0, 1.0);
            gl_api.clear(gl::COLOR_BUFFER_BIT);

            gl_api.viewport(
                0,
                0,
                physical_size.x as gl::GLsizei,
                physical_size.y as gl::GLsizei,
            );

            gl_api.enable(gl::BLEND);
            gl_api.blend_equation(gl::FUNC_ADD);
            gl_api.blend_func_separate(
                gl::SRC_ALPHA,
                gl::ONE_MINUS_SRC_ALPHA,
                gl::ONE,
                gl::ONE_MINUS_SRC_ALPHA,
            );

            gl_api.bind_vertex_array(Some(self.vao));

            // TODO: should probably do buffer_sub_data here?

            let positions = attributes
                .get(&sx::VertexAttribute::POSITION2)
                .context("could not get position2")?;
            gl_api.bind_buffer(gl::ARRAY_BUFFER, Some(self.vbo_positions));
            gl_api.buffer_data(
                gl::ARRAY_BUFFER,
                (size_of::<[f32; 2]>() * positions.len()) as gl::GLsizeiptr,
                positions.as_bytes().as_ptr().cast(),
                gl::STREAM_DRAW,
            );

            let colors = attributes
                .get(&sx::VertexAttribute::COLOR)
                .context("could not get colors")?;
            gl_api.bind_buffer(gl::ARRAY_BUFFER, Some(self.vbo_colors));
            gl_api.buffer_data(
                gl::ARRAY_BUFFER,
                (size_of::<[u32; 4]>() * colors.len()) as gl::GLsizeiptr,
                colors.as_bytes().as_ptr().cast(),
                gl::STREAM_DRAW,
            );

            let uvs = attributes
                .get(&sx::VertexAttribute::UV2)
                .context("could not get uv2")?;
            gl_api.bind_buffer(gl::ARRAY_BUFFER, Some(self.vbo_uvs));
            gl_api.buffer_data(
                gl::ARRAY_BUFFER,
                (size_of::<[f32; 2]>() * uvs.len()) as gl::GLsizeiptr,
                uvs.as_bytes().as_ptr().cast(),
                gl::STREAM_DRAW,
            );

            gl_api.bind_buffer(gl::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
            gl_api.buffer_data(
                gl::ELEMENT_ARRAY_BUFFER,
                (size_of::<u32>() * indices.len()) as gl::GLsizeiptr,
                indices.as_ptr().cast(),
                gl::STREAM_DRAW,
            );
        }

        for sx::DrawCommand {
            index_range,
            scissor,
            material,
        } in commands.iter()
        {
            let _maybe_scissor_guard = scissor.map(|logical_rect| {
                // NOTE: scissor needs to be aware of y flip.
                //   projection matrix does not flip y, it's opengl-specific.
                //   glBlitFramebuffer flips y.
                //
                // NOTE: everything on the cpu is in @LogicalPixels.
                //   scissor rect needs to be scaled.
                let physical_rect = logical_rect.scale(scale_factor);
                let x = physical_rect.min.x as i32;
                let y = physical_rect.min.y as i32;
                let w = physical_rect.width() as i32;
                let h = physical_rect.height() as i32;
                unsafe {
                    gl_api.enable(gl::SCISSOR_TEST);
                    gl_api.scissor(x, y, w, h);
                }
                ScopeGuard::new(|| unsafe { gl_api.disable(gl::SCISSOR_TEST) })
            });

            let shader = self
                .shaders
                .get(&material.pipeline)
                .with_context(|| format!("could not get pipeline (id {:?})", material.pipeline))?;

            unsafe {
                gl_api.use_program(Some(shader.gl_handle));

                for (name, location) in shader.uniform_locations.0.iter() {
                    match name.as_str() {
                        "projection_matrix" => {
                            gl_api.uniform_matrix_4fv(
                                *location,
                                1,
                                gl::FALSE,
                                projection_matrix.as_ptr().cast(),
                            );
                        }
                        "scale_factor" => {
                            gl_api.uniform_1f(*location, scale_factor);
                        }
                        other => {
                            if let Some(value) = material.get_uniform(other) {
                                use sx::UniformValue::*;
                                match value {
                                    Int(v) => {
                                        gl_api.uniform_1i(*location, v);
                                    }
                                    Float(v) => {
                                        gl_api.uniform_1f(*location, v);
                                    }
                                    Float2(v) => {
                                        gl_api.uniform_2f(*location, v[0], v[1]);
                                    }
                                    Float4(v) => {
                                        gl_api.uniform_4f(*location, v[0], v[1], v[2], v[3]);
                                    }
                                    Mat4(v) => {
                                        gl_api.uniform_matrix_4fv(
                                            *location,
                                            1,
                                            gl::FALSE,
                                            &v as *const _ as _,
                                        );
                                    }
                                    _ => {
                                        return Err(anyhow!("unhandled uniform {value:?}"));
                                    }
                                }
                            }
                        }
                    }
                }

                for (name, unit) in shader.texture_units.0.iter() {
                    let texture = if let Some(handle) = material.get_texture(name) {
                        self.textures.get(&handle)
                    } else {
                        None
                    }
                    .unwrap_or(&self.default_white_texture);
                    gl_api.active_texture(*unit);
                    gl_api.bind_texture(gl::TEXTURE_2D, Some(texture.gl_handle));
                }

                gl_api.draw_elements(
                    gl::TRIANGLES,
                    index_range.len() as gl::GLsizei,
                    gl::UNSIGNED_INT,
                    (index_range.start * size_of::<u32>() as u32) as *const c_void,
                );
            }
        }

        Ok(())
    }

    pub fn render_to_screen(&self, gl_api: &gl::wrap::Api) -> anyhow::Result<()> {
        let Some(framebuffer) = self.framebuffer.as_ref() else {
            return Ok(());
        };

        unsafe {
            gl_api.bind_framebuffer(gl::DRAW_FRAMEBUFFER, None);
            // NOTE: draw buffer needs to be specififed.
            //   without i don't see anything being rendered on nvidia gpu,
            //   but on amd gpu it's fine.
            gl_api.draw_buffer(gl::BACK);

            gl_api.bind_framebuffer(gl::READ_FRAMEBUFFER, Some(framebuffer.framebuffer));
            gl_api.read_buffer(gl::COLOR_ATTACHMENT0);

            // NOTE: this flips y.
            gl_api.blit_framebuffer(
                0,
                0,
                framebuffer.width as gl::GLint,
                framebuffer.height as gl::GLint,
                0,
                framebuffer.height as gl::GLint,
                framebuffer.width as gl::GLint,
                0,
                gl::COLOR_BUFFER_BIT,
                gl::NEAREST,
            );
        }

        Ok(())
    }
}
