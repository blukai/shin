use std::collections::hash_map;
use std::ffi::c_void;
use std::fmt::{self, Write as _};
use std::hash::Hasher as _;
use std::mem::offset_of;
use std::ops;
use std::ptr::null;

use anyhow::{Context as _, anyhow};
use gl::wrap::Adapter;
use mars::alloc::{self, Allocator, TempAllocator};
use mars::fxhash::FxHasher;
use mars::memory::GrowableMemory;
use mars::nohash::NoHashMap;
use mars::scopeguard::ScopeGuard;
use mars::string::{GrowableString, String};
use mars::vector::SpillableVector;

// TODO: maybe ubo

struct ShaderUniformLocation {
    name: sx::ShaderUniformName,
    location: gl::wrap::UniformLocation,
}

#[derive(Default)]
struct ShaderUniformLocations(
    SpillableVector<ShaderUniformLocation, { sx::INITIAL_SHADER_UNIFORMS_CAP }, alloc::Global>,
);

impl ShaderUniformLocations {
    fn set(&mut self, name: &str, location: gl::wrap::UniformLocation) {
        for it in self.0.iter_mut() {
            if it.name == name {
                it.location = location;
                return;
            }
        }
        self.0.push(ShaderUniformLocation {
            name: sx::ShaderUniformName::from_str(name),
            location,
        });
    }
}

impl ops::Deref for ShaderUniformLocations {
    type Target = [ShaderUniformLocation];

    fn deref(&self) -> &Self::Target {
        self.0.as_slice()
    }
}

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

#if defined(ROUNDED_RECT_SDF)
uniform vec2 u_rect_center;
uniform vec2 u_rect_half_size;
uniform float u_rect_corner_radius;

// https://www.shadertoy.com/view/WtdSDs
// https://iquilezles.org/articles/distfunctions2d/
float rounded_rect_sdf(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
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

#if defined(ROUNDED_RECT_SDF)
    const float EDGE_SOFTNESS = 1.0;
    float distance = rounded_rect_sdf(gl_FragCoord.xy - u_rect_center, u_rect_half_size, u_rect_corner_radius);
    // outer edge, full shape
    float shape_alpha = 1.0 - smoothstep(0.0, EDGE_SOFTNESS, distance);
    FragColor.a *= shape_alpha;

// TODO: inner/center/outer stroke.
// TODO: stroke uniforms.
// TODO: investigate more weird blending, it's visible;
//   see: https://www.redblobgames.com/blog/2024-08-27-sdf-font-outlines/.

// float border_thickness = 8.0;
// vec4 border_color = vec4(1.0, 1.0, 1.0, 0.5);
//
// // inner edge, fill area, inset by border_thickness
// float fill_alpha = 1.0 - smoothstep(0.0, EDGE_SOFTNESS, distance + border_thickness);
// float border_alpha = shape_alpha - fill_alpha;
//
// FragColor = mix(FragColor, border_color, border_alpha);
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
        .iter()
        .chain(shader_desc.fragment_stage.defines.iter())
    {
        hasher.write(define.as_bytes())
    }
    hasher.finish()
}

struct Shader {
    gl_handle: gl::wrap::Program,
    uniform_locations: ShaderUniformLocations,
}

impl Shader {
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
            .with_cap(code.len() + defines.len() * APPROX_DEFINE_LEN + APPROX_EXTRA_CAP);

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

        for define in defines.iter() {
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
            let vert_shader = Self::create_shader(gl_api, vert_src, gl::VERTEX_SHADER)?;
            let frag_shader = Self::create_shader(gl_api, frag_src, gl::FRAGMENT_SHADER)?;

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

    fn new(
        desc: &sx::ShaderDesc,
        gl_api: &gl::wrap::Api,
        temp: &TempAllocator<'_>,
    ) -> anyhow::Result<Self> {
        let program = unsafe {
            let _temp_guard = temp.scope_guard();
            Self::create_program(
                gl_api,
                &Self::prefix_stage_source(&desc.vertex_stage, temp)
                    .context("could not prefix vertex shader")?,
                &Self::prefix_stage_source(&desc.fragment_stage, temp)
                    .context("could not prefix fragment shader")?,
            )
            .context("could not create rgba8 program")?
        };

        let mut uniform_locations = ShaderUniformLocations::default();
        for desc in desc.uniforms.iter() {
            let _temp_guard = temp.scope_guard();
            let name = desc
                .name
                .try_to_c_string_in(GrowableMemory::new_in(temp))
                .expect("could not convier uniform name to cstring");
            if let Some(location) = unsafe { gl_api.get_uniform_location(program, name.as_c_str()) }
            {
                uniform_locations.set(&desc.name, location);
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

    shaders: NoHashMap<u64, Shader>,

    default_white_texture: Texture,
    textures: NoHashMap<sx::TextureHandle, Texture>,
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

                shaders: NoHashMap::default(),

                default_white_texture: create_default_white_texture(gl_api)
                    .context("could not create default white tex")?,
                textures: NoHashMap::default(),
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
                uniforms,
                scissor,
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
                        defines: sx::ShaderDefines::default().and_set("SHADER_STAGE_VERTEX"),
                    },
                    fragment_stage: sx::ShaderStageDesc {
                        source: SHADER_SOURCE.clone(),
                        defines: sx::ShaderDefines::default().and_set("SHADER_STAGE_FRAGMENT"),
                    },
                    uniforms: sx::ShaderUniformDescs::default()
                        .and_set("u_projection", sx::ShaderUniformType::Mat4)
                        // TODO: figure out situation of u_texture -> u_sampler.
                        .and_set("u_sampler", sx::ShaderUniformType::Sampler2D),
                };

                let mut texture = &self.default_white_texture;
                let mut maybe_rect_center = None::<sx::Vec2>;
                let mut maybe_rect_half_size = None::<sx::Vec2>;
                let mut maybe_rect_corner_radius = None::<f32>;
                if let Some(mut uniforms) = uniforms {
                    // TODO: impl vector into iter.
                    for u in uniforms.0.drain(..) {
                        use sx::ShaderUniformType as Type;
                        use sx::ShaderUniformValue as Value;
                        // TODO: should be able set with owned name without a hop.
                        match (u.name.as_str(), u.value) {
                            ("u_texture", Value::Texture2D(h)) => {
                                texture = self.textures.get(&h).expect("invalid texture handle");
                            }

                            (name @ "u_rect_center", Value::Vec2(center)) => {
                                maybe_rect_center = Some(center);
                                shader_desc.uniforms.set(name, Type::Vec2);
                            }
                            (name @ "u_rect_half_size", Value::Vec2(half_size)) => {
                                maybe_rect_half_size = Some(half_size);
                                shader_desc.uniforms.set(name, Type::Vec2);
                            }
                            (name @ "u_rect_corner_radius", Value::Float(corner_radius)) => {
                                maybe_rect_corner_radius = Some(corner_radius);
                                shader_desc.uniforms.set(name, Type::Float);
                            }

                            other => {
                                log::warn!("unhandled uniform: {other:?}");
                            }
                        }
                    }
                }
                shader_desc
                    .fragment_stage
                    .defines
                    .set(match texture.format {
                        sx::TextureFormat::Rgba8Unorm => "TEXTURE_FORMAT_RGBA8",
                        sx::TextureFormat::R8Unorm => "TEXTURE_FORMAT_R8",
                    });

                match (
                    maybe_rect_center,
                    maybe_rect_half_size,
                    maybe_rect_corner_radius,
                ) {
                    (None, None, None) => {}
                    (Some(..), Some(..), Some(..)) => {
                        shader_desc.fragment_stage.defines.set("ROUNDED_RECT_SDF");
                    }
                    incomplete => {
                        log::warn!("incomplete set of rounded rect uniforms: {incomplete:?}");
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

                    for ul in shader.uniform_locations.iter() {
                        // NOTE: everything on the cpu is in @LogicalPixels.
                        //   uniforms that carry size/coord data must be scaled.
                        match ul.name.as_str() {
                            "u_projection" => {
                                gl_api.uniform_matrix_4fv(
                                    ul.location,
                                    1,
                                    gl::FALSE,
                                    projection_matrix.as_ptr().cast(),
                                );
                            }
                            "u_sampler" => {
                                gl_api.uniform_1i(ul.location, 0);

                                gl_api.active_texture(gl::TEXTURE0);
                                gl_api.bind_texture(gl::TEXTURE_2D, Some(texture.gl_handle));
                            }

                            "u_rect_center" => {
                                let center = maybe_rect_center.unwrap() * scale_factor;
                                gl_api.uniform_2f(ul.location, center.x, center.y);
                            }
                            "u_rect_half_size" => {
                                let half_size = maybe_rect_half_size.unwrap() * scale_factor;
                                gl_api.uniform_2f(ul.location, half_size.x, half_size.y);
                            }
                            "u_rect_corner_radius" => {
                                let corner_radius =
                                    maybe_rect_corner_radius.unwrap() * scale_factor;
                                gl_api.uniform_1f(ul.location, corner_radius);
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
