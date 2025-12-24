use std::collections::{HashMap, hash_map};
use std::ffi::c_void;
use std::fmt::{self, Write as _};
use std::hash::Hasher as _;
use std::mem::offset_of;
use std::ptr::null;

use anyhow::{Context as _, anyhow};
use gl::wrap::Adapter;
use mars::alloc::{self, Allocator, TempAllocator};
use mars::fxhash::FxHasher;
use mars::memory::GrowableMemory;
use mars::nohash::NoBuildHasher;
use mars::scopeguard::ScopeGuard;
use mars::sortedarray::SpillableSortedArrayMap;
use mars::string::{GrowableString, String};

// TODO: maybe ubo
// TODO: do i want to generate uniforms from shader desc?
// TODO: do i want to generate vertex input state from what? primitive attributes or something?

type ShaderUniformLocations = SpillableSortedArrayMap<
    sx::ShaderUniformName,
    gl::wrap::UniformLocation,
    { sx::INITIAL_SHADER_UNIFORMS_CAP },
    alloc::Global,
>;

// NOTE: some kind of naming conventions for shader things
//   - `a_` for attributes
//   - `v_` for vertex-to-fragment outputs
//   - `u_` for uniforms

const A_POSITION_LOC: gl::GLuint = 0;
const A_TEX_COORD_LOC: gl::GLuint = 1;
const A_COLOR_LOC: gl::GLuint = 2;

const SHADER: &str = "
#if defined(SHADER_STAGE_VERTEX)
layout(location = 0) in vec2 a_position;
layout(location = 1) in vec2 a_tex_coord;
layout(location = 2) in vec4 a_color;

uniform mat4 u_projection;

out vec2 v_tex_coord;
out vec4 v_color;

void main() {
    v_tex_coord = a_tex_coord;
    v_color = a_color / 255.0; // normalize 0..255 to 0.0..1.0
    gl_Position = u_projection * vec4(a_position, 0.0, 1.0);
}
#endif

#if defined(SHADER_STAGE_FRAGMENT)
in vec2 v_tex_coord;
in vec4 v_color;

uniform sampler2D u_sampler;

#if defined(SDF_RECT)
uniform vec2 u_sdf_rect_center;
uniform vec2 u_sdf_rect_size;
uniform float u_sdf_rect_corner_radius;
uniform float u_sdf_rect_stroke_width;
uniform vec4 u_sdf_rect_stroke_color;
uniform int u_sdf_rect_stroke_alignment; // -1 inside, 0 center, 1 outside

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
    vec2 rect_center,
    vec2 rect_size,
    float corner_radius,
    float stroke_width,
    int stroke_alignment,
    vec4 stroke_color
) {
    vec2 stroke_split = split_stroke(stroke_width, stroke_alignment);
    float stroke_inner = stroke_split.x;
    float stroke_outer = stroke_split.y;

    vec2 p = frag_pos - rect_center;
    vec2 b = rect_size * 0.5 + stroke_outer;
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
#endif

out vec4 FragColor;

void main() {
    FragColor = v_color;

#if defined(TEXTURE_FORMAT_R8)
    FragColor.a *= texture(u_sampler, v_tex_coord).r;
#elif defined(TEXTURE_FORMAT_RGBA8)
    FragColor *= texture(u_sampler, v_tex_coord);
#endif

#if defined(SDF_RECT)
    FragColor = sdf_rect(
        gl_FragCoord.xy,
        FragColor,
        u_sdf_rect_center,
        u_sdf_rect_size,
        u_sdf_rect_corner_radius,
        u_sdf_rect_stroke_width,
        u_sdf_rect_stroke_alignment,
        u_sdf_rect_stroke_color
    );
#endif
}
#endif
";

const SHADER_SOURCE: sx::ShaderSource = sx::ShaderSource {
    kind: sx::ShaderSourceKind::Static(SHADER),
    desc: if cfg!(target_family = "wasm") {
        sx::ShaderSourceDesc::Glsl {
            version: 300,
            profile: sx::GlslProfile::Es,
        }
    } else {
        sx::ShaderSourceDesc::Glsl {
            version: 330,
            profile: sx::GlslProfile::Core,
        }
    },
};

fn hash_shader_desc(shader_desc: &sx::ShaderDesc) -> u64 {
    // NOTE: source doesn't change. uniforms don't affect anything.
    //   what we care about is defines.
    let mut hasher = FxHasher::default();
    for define in shader_desc
        .vertex_stage
        .defines
        .0
        .iter()
        .chain(shader_desc.fragment_stage.defines.0.iter())
    {
        hasher.write(define.as_bytes())
    }
    hasher.finish()
}

fn prefix_stage_source<A: Allocator>(
    stage_desc: &sx::ShaderStageDesc,
    alloc: A,
) -> Result<GrowableString<A>, fmt::Error> {
    let sx::ShaderStageDesc {
        source:
            sx::ShaderSource {
                kind: sx::ShaderSourceKind::Static(code),
                desc: sx::ShaderSourceDesc::Glsl { version, profile },
            },
        defines,
    } = stage_desc;

    const APPROX_DEFINE_LEN: usize = 32;
    const APPROX_EXTRA_CAP: usize = 128;
    let mut ret = String::new_growable_in(alloc)
        .with_cap(code.len() + defines.0.len() * APPROX_DEFINE_LEN + APPROX_EXTRA_CAP);

    match (version, profile) {
        (version, sx::GlslProfile::Core) => {
            ret.write_fmt(format_args!("#version {version} core\n"))?;
        }
        (_version, sx::GlslProfile::Compatibility) => {
            unimplemented!()
        }
        (version, sx::GlslProfile::Es) => {
            ret.write_fmt(format_args!("#version {version} es\n"))?;
            // NOTE: type can only be float or int.
            //   see https://wikis.khronos.org/opengl/Type_Qualifier_(GLSL)#Precision_qualifiers
            ret.write_fmt(format_args!("precision highp float;\n"))?;
            ret.write_fmt(format_args!("precision highp int;\n"))?;
        }
    }

    for define in defines.0.iter() {
        ret.write_fmt(format_args!("#define {name}\n", name = define.as_str()))?;
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
    uniform_locations: ShaderUniformLocations,
}

impl Shader {
    fn new(
        desc: &sx::ShaderDesc,
        gl_api: &gl::wrap::Api,
        temp: &TempAllocator<'_>,
    ) -> anyhow::Result<Self> {
        let program = unsafe {
            let _temp_guard = temp.scope_guard();
            create_program(
                gl_api,
                &prefix_stage_source(&desc.vertex_stage, temp)
                    .context("could not prefix vertex shader")?,
                &prefix_stage_source(&desc.fragment_stage, temp)
                    .context("could not prefix fragment shader")?,
            )
            .context("could not create rgba8 program")?
        };

        let mut uniform_locations = ShaderUniformLocations::default();
        for (name, _) in desc.uniforms.0.iter() {
            if let Some(location) = {
                let _temp_guard = temp.scope_guard();
                let name = name
                    .try_to_c_string_in(GrowableMemory::new_in(temp))
                    .expect("could not convier uniform name to cstring");
                unsafe { gl_api.get_uniform_location(program, name.as_c_str()) }
            } {
                uniform_locations.insert(name.clone(), location);
            }
        }

        Ok(Shader {
            gl_handle: program,
            uniform_locations,
        })
    }
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

    vbo: gl::wrap::Buffer,
    ebo: gl::wrap::Buffer,
    vao: gl::wrap::VertexArray,

    shaders: HashMap<u64, Shader, NoBuildHasher<u64>>,

    default_white_texture: Texture,
    textures: HashMap<sx::TextureHandle, Texture, NoBuildHasher<sx::TextureHandle>>,
}

impl GlRenderer {
    pub fn new(gl_api: &gl::wrap::Api) -> anyhow::Result<Self> {
        // TODO: scopeguard to cleanup created resources if something fails
        //   via checkpoint-based gl allocator

        unsafe {
            let vbo = gl_api.create_buffer().context("could not create vbo")?;
            let ebo = gl_api.create_buffer().context("could not create ebo")?;
            let vao = {
                let vao = gl_api
                    .create_vertex_array()
                    .context("could not create vao")?;

                gl_api.bind_vertex_array(Some(vao));
                gl_api.bind_buffer(gl::ARRAY_BUFFER, Some(vbo));

                const STRIDE: gl::GLsizei = size_of::<sx::Vertex>() as gl::GLsizei;

                gl_api.vertex_attrib_pointer(
                    A_POSITION_LOC,
                    2,
                    gl::FLOAT,
                    gl::FALSE,
                    STRIDE,
                    offset_of!(sx::Vertex, position) as *const c_void,
                );
                gl_api.enable_vertex_attrib_array(A_POSITION_LOC);

                gl_api.vertex_attrib_pointer(
                    A_TEX_COORD_LOC,
                    2,
                    gl::FLOAT,
                    gl::FALSE,
                    STRIDE,
                    offset_of!(sx::Vertex, tex_coord) as *const c_void,
                );
                gl_api.enable_vertex_attrib_array(A_TEX_COORD_LOC);

                gl_api.vertex_attrib_pointer(
                    A_COLOR_LOC,
                    4,
                    gl::UNSIGNED_BYTE,
                    gl::FALSE,
                    STRIDE,
                    offset_of!(sx::Vertex, color) as *const c_void,
                );
                gl_api.enable_vertex_attrib_array(A_COLOR_LOC);

                vao
            };

            Ok(Self {
                framebuffer: None,

                vbo,
                ebo,
                vao,

                shaders: HashMap::default(),

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

            gl_api.delete_buffer(self.vbo);
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

    pub fn render<'a>(
        &mut self,
        logical_size: sx::Vec2,
        scale_factor: f32,
        draw_layers: sx::DrawLayersDrain<'_>,
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

            gl_api.bind_buffer(gl::ARRAY_BUFFER, Some(self.vbo));
            gl_api.bind_buffer(gl::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
            gl_api.bind_vertex_array(Some(self.vao));
        }

        for sx::DrawLayerFlush {
            vertices,
            indices,
            commands,
        } in draw_layers
        {
            unsafe {
                // TODO: should probably do buffer_sub_data here?
                gl_api.buffer_data(
                    gl::ARRAY_BUFFER,
                    (vertices.len() * size_of::<sx::Vertex>()) as gl::GLsizeiptr,
                    vertices.as_ptr().cast(),
                    gl::STREAM_DRAW,
                );
                gl_api.buffer_data(
                    gl::ELEMENT_ARRAY_BUFFER,
                    (indices.len() * size_of::<u32>()) as gl::GLsizeiptr,
                    indices.as_ptr().cast(),
                    gl::STREAM_DRAW,
                );
            }

            for sx::DrawCommand {
                index_range,
                texture,
                scissor,
                sdf_params,
            } in commands
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

                let mut shader_desc = sx::ShaderDesc {
                    vertex_stage: sx::ShaderStageDesc {
                        source: SHADER_SOURCE.clone(),
                        defines: sx::ShaderDefines::default().with_iter(
                            [sx::ShaderDefine::new_fixed().with_str("SHADER_STAGE_VERTEX")]
                                .into_iter(),
                        ),
                    },
                    fragment_stage: sx::ShaderStageDesc {
                        source: SHADER_SOURCE.clone(),
                        defines: sx::ShaderDefines::default().with_iter(
                            [sx::ShaderDefine::new_fixed().with_str("SHADER_STAGE_FRAGMENT")]
                                .into_iter(),
                        ),
                    },
                    uniforms: sx::ShaderUniformDescs::default().with_iter(
                        [
                            (
                                sx::ShaderUniformName::new_fixed().with_str("u_projection"),
                                sx::ShaderUniformType::Mat4,
                            ),
                            (
                                sx::ShaderUniformName::new_fixed().with_str("u_sampler"),
                                sx::ShaderUniformType::Sampler2D,
                            ),
                        ]
                        .into_iter(),
                    ),
                };

                let texture = if let Some(handle) = texture {
                    self.textures.get(&handle).expect("invalid handle")
                } else {
                    &self.default_white_texture
                };
                shader_desc
                    .fragment_stage
                    .defines
                    .insert(
                        sx::ShaderDefine::new_fixed().with_str(match texture.format {
                            sx::TextureFormat::Rgba8Unorm => "TEXTURE_FORMAT_RGBA8",
                            sx::TextureFormat::R8Unorm => "TEXTURE_FORMAT_R8",
                        }),
                    );

                if let Some(sdf_params) = sdf_params.as_ref() {
                    use sx::ShaderUniformType as Type;
                    match sdf_params {
                        sx::SdfParams::Rect(..) => {
                            shader_desc.fragment_stage.defines.extend_from_iter(
                                [sx::ShaderDefine::new_fixed().with_str("SDF_RECT")].into_iter(),
                            );

                            shader_desc.uniforms.extend_from_iter(
                                [
                                    (
                                        sx::ShaderUniformName::new_fixed()
                                            .with_str("u_sdf_rect_center"),
                                        Type::Vec2,
                                    ),
                                    (
                                        sx::ShaderUniformName::new_fixed()
                                            .with_str("u_sdf_rect_size"),
                                        Type::Vec2,
                                    ),
                                    (
                                        sx::ShaderUniformName::new_fixed()
                                            .with_str("u_sdf_rect_corner_radius"),
                                        Type::Float,
                                    ),
                                    (
                                        sx::ShaderUniformName::new_fixed()
                                            .with_str("u_sdf_rect_stroke_width"),
                                        Type::Float,
                                    ),
                                    (
                                        sx::ShaderUniformName::new_fixed()
                                            .with_str("u_sdf_rect_stroke_color"),
                                        Type::Vec4,
                                    ),
                                    (
                                        sx::ShaderUniformName::new_fixed()
                                            .with_str("u_sdf_rect_stroke_alignment"),
                                        Type::Int,
                                    ),
                                ]
                                .into_iter(),
                            );
                        }
                    }
                }

                let shader_key = hash_shader_desc(&shader_desc);
                let shader = match self.shaders.entry(shader_key) {
                    hash_map::Entry::Occupied(x) => x.into_mut(),
                    hash_map::Entry::Vacant(x) => {
                        let shader =
                            Shader::new(&shader_desc, gl_api, temp).with_context(|| {
                                // TODO: can i use temp alloc here? it needs to be send+sync for some
                                // fucking reason.
                                std::format!("could not create shader\n{shader_desc:?}")
                            })?;
                        x.insert(shader)
                    }
                };

                unsafe {
                    gl_api.use_program(Some(shader.gl_handle));

                    // NOTE: everything on the cpu is in @LogicalPixels.
                    //   uniforms that carry size/coord data must be scaled.
                    //
                    // NOTE: this is somewhat awkward,
                    //   but still i prefer this loop over individual lookups for each loc.
                    for (name, location) in shader.uniform_locations.0.iter() {
                        match name.as_str() {
                            "u_projection" => {
                                gl_api.uniform_matrix_4fv(
                                    *location,
                                    1,
                                    gl::FALSE,
                                    projection_matrix.as_ptr().cast(),
                                );

                                gl_api.active_texture(gl::TEXTURE0);
                                gl_api.bind_texture(gl::TEXTURE_2D, Some(texture.gl_handle));
                                gl_api.uniform_1i(*location, 0);
                            }
                            "u_sampler" => {
                                gl_api.active_texture(gl::TEXTURE0);
                                gl_api.bind_texture(gl::TEXTURE_2D, Some(texture.gl_handle));
                                gl_api.uniform_1i(*location, 0);
                            }

                            "u_sdf_rect_center" => {
                                let Some(sx::SdfParams::Rect(rect_sdf)) = sdf_params.as_ref()
                                else {
                                    unreachable!();
                                };
                                let center = rect_sdf.center * scale_factor;
                                gl_api.uniform_2f(*location, center.x, center.y);
                            }
                            "u_sdf_rect_size" => {
                                let Some(sx::SdfParams::Rect(rect_sdf)) = sdf_params.as_ref()
                                else {
                                    unreachable!();
                                };
                                let size = rect_sdf.size * scale_factor;
                                gl_api.uniform_2f(*location, size.x, size.y);
                            }
                            "u_sdf_rect_corner_radius" => {
                                let Some(sx::SdfParams::Rect(rect_sdf)) = sdf_params.as_ref()
                                else {
                                    unreachable!();
                                };
                                let corner_radius =
                                    rect_sdf.corner_radius.unwrap_or(0.0) * scale_factor;
                                gl_api.uniform_1f(*location, corner_radius);
                            }
                            "u_sdf_rect_stroke_width" => {
                                let Some(sx::SdfParams::Rect(rect_sdf)) = sdf_params.as_ref()
                                else {
                                    unreachable!();
                                };
                                let stroke_width = if let Some(ref stroke) = rect_sdf.stroke {
                                    stroke.width
                                } else {
                                    0.0
                                } * scale_factor;
                                gl_api.uniform_1f(*location, stroke_width);
                            }
                            "u_sdf_rect_stroke_color" => {
                                let Some(sx::SdfParams::Rect(rect_sdf)) = sdf_params.as_ref()
                                else {
                                    unreachable!();
                                };
                                let c = if let Some(ref stroke) = rect_sdf.stroke {
                                    stroke.color
                                } else {
                                    sx::Rgba8::TRANSPARENT
                                }
                                .to_f32_array();
                                gl_api.uniform_4f(*location, c[0], c[1], c[2], c[3]);
                            }
                            "u_sdf_rect_stroke_alignment" => {
                                let Some(sx::SdfParams::Rect(rect_sdf)) = sdf_params.as_ref()
                                else {
                                    unreachable!();
                                };
                                // NOTE: stroke alignment value convention must be in-sync with the
                                // shader.
                                let stroke_alignment = if let Some(ref stroke) = rect_sdf.stroke {
                                    match stroke.alignment {
                                        sx::StrokeAlignment::Inside => -1,
                                        sx::StrokeAlignment::Outside => 1,
                                        sx::StrokeAlignment::Center => 0,
                                    }
                                } else {
                                    0
                                };
                                gl_api.uniform_1i(*location, stroke_alignment);
                            }

                            other => {
                                log::warn!("uniform {other} was left unset");
                            }
                        }
                    }

                    gl_api.draw_elements(
                        gl::TRIANGLES,
                        index_range.len() as gl::GLsizei,
                        gl::UNSIGNED_INT,
                        (index_range.start * size_of::<u32>() as u32) as *const c_void,
                    );
                }
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
