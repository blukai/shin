use std::ffi::c_void;
use std::mem::offset_of;
use std::ptr::null;

use anyhow::{Context as _, anyhow};
use gl::wrap::Adapter;
use nohash::NoHashMap;
use sx::TextureFormat;

// NOTE: some kind of naming conventions for shader things
//   - `a_` for attributes
//   - `v_` for vertex-to-fragment outputs
//   - `u_` for uniforms
//   - `fragColor` for fragment output

const A_POSITION_LOC: gl::GLuint = 0;
const A_TEX_COORD_LOC: gl::GLuint = 1;
const A_COLOR_LOC: gl::GLuint = 2;

const SHADER: &str = "
#if defined(VERTEX_SHADER)
in vec2 a_position;
in vec2 a_tex_coord;
in vec4 a_color;

uniform mat4 u_projection;

out vec2 v_tex_coord;
out vec4 v_color;

void main() {
    v_tex_coord = a_tex_coord;
    v_color = a_color / 255.0; // normalize 0..255 to 0.0..1.0
    gl_Position = u_projection * vec4(a_position, 0.0, 1.0);
}
#endif

#if defined(FRAGMENT_SHADER)
in vec2 v_tex_coord;
in vec4 v_color;

uniform sampler2D u_sampler;

out vec4 fragColor;

void main() {
    vec4 texture_sample = texture(u_sampler, v_tex_coord);
#if defined(TEXTURE_FORMAT_R8)
    fragColor = vec4(v_color.rgb, v_color.a * texture_sample.r);
#else // TEXTURE_FORMAT_RGBA8 is the default
    fragColor = v_color * texture_sample;
#endif
}
#endif
";

fn prefix_vertex_shader(shader: &str) -> String {
    let mut ret = String::new();

    if cfg!(target_family = "wasm") {
        ret.push_str("#version 300 es\n");
        ret.push_str("precision mediump float;\n");
    } else {
        ret.push_str("#version 330 core\n");
    }

    ret.push_str("#define VERTEX_SHADER\n");

    ret.push_str(shader);
    ret
}

fn prefix_fragment_shader(shader: &str, tex_format: sx::TextureFormat) -> String {
    let mut ret = String::new();

    if cfg!(target_family = "wasm") {
        ret.push_str("#version 300 es\n");
        ret.push_str("precision mediump float;\n");
    } else {
        ret.push_str("#version 330 core\n");
    }

    ret.push_str("#define FRAGMENT_SHADER\n");

    match tex_format {
        sx::TextureFormat::Rgba8Unorm => ret.push_str("#define TEXTURE_FORMAT_RGBA8\n"),
        sx::TextureFormat::R8Unorm => ret.push_str("#define TEXTURE_FORMAT_R8\n"),
    }

    ret.push_str(shader);
    ret
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
    program: gl::wrap::Program,
    u_projection_loc: gl::wrap::UniformLocation,
    u_sampler_loc: gl::wrap::UniformLocation,
}

struct TextureFormatDesc {
    internal_format: gl::GLint,
    format: gl::GLenum,
    ty: gl::GLenum,
    // like https://docs.vulkan.org/spec/latest/chapters/formats.html#texel-block-size
    block_size: gl::GLint,
}

fn describe_texture_format(format: TextureFormat) -> TextureFormatDesc {
    match format {
        TextureFormat::Rgba8Unorm => TextureFormatDesc {
            internal_format: gl::RGBA8 as _,
            format: gl::RGBA,
            ty: gl::UNSIGNED_BYTE,
            block_size: 4,
        },
        TextureFormat::R8Unorm => TextureFormatDesc {
            internal_format: gl::R8 as _,
            format: gl::RED,
            ty: gl::UNSIGNED_BYTE,
            block_size: 1,
        },
    }
}

struct Texture {
    gl_handle: gl::wrap::Texture,
    format: TextureFormat,
}

unsafe fn create_default_white_texture(gl_api: &gl::wrap::Api) -> anyhow::Result<Texture> {
    unsafe {
        let texture = gl_api
            .create_texture()
            .context("could not create texture")?;
        gl_api.bind_texture(gl::TEXTURE_2D, Some(texture));

        gl_api.tex_image_2d(
            gl::TEXTURE_2D,
            0,
            gl::RGBA8 as gl::GLint,
            1,
            1,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            [255_u8; 4].as_ptr().cast(),
        );

        Ok(Texture {
            gl_handle: texture,
            format: TextureFormat::Rgba8Unorm,
        })
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
    shader_rgba8: Shader,
    shader_r8: Shader,
    active_program: Option<gl::wrap::Program>,

    vbo: gl::wrap::Buffer,
    ebo: gl::wrap::Buffer,
    vao: gl::wrap::VertexArray,

    default_white_texture: Texture,
    textures: NoHashMap<sx::TextureHandle, Texture>,
}

impl sx::Externs for GlRenderer {
    type TextureHandle = gl::wrap::Texture;
}

impl GlRenderer {
    pub fn new(gl_api: &gl::wrap::Api) -> anyhow::Result<Self> {
        unsafe {
            let shader_rgba8 = {
                let program = create_program(
                    gl_api,
                    &prefix_vertex_shader(SHADER),
                    &prefix_fragment_shader(SHADER, sx::TextureFormat::Rgba8Unorm),
                )
                .context("could not create rgba8 program")?;
                let u_projection_loc = gl_api
                    .get_uniform_location(program, c"u_projection")
                    .context("could not get loc of u_projection")?;
                let u_sampler_loc = gl_api
                    .get_uniform_location(program, c"u_sampler")
                    .context("could not get loc of u_sampler")?;
                Shader {
                    program,
                    u_projection_loc,
                    u_sampler_loc,
                }
            };
            let shader_r8 = {
                let program = create_program(
                    gl_api,
                    &prefix_vertex_shader(SHADER),
                    &prefix_fragment_shader(SHADER, sx::TextureFormat::R8Unorm),
                )
                .context("could not create r8 program")?;
                let u_projection_loc = gl_api
                    .get_uniform_location(program, c"u_projection")
                    .context("could not get loc of u_projection")?;
                let u_sampler_loc = gl_api
                    .get_uniform_location(program, c"u_sampler")
                    .context("could not get loc of u_sampler")?;
                Shader {
                    program,
                    u_projection_loc,
                    u_sampler_loc,
                }
            };

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
                shader_rgba8,
                shader_r8,
                active_program: None,

                vbo,
                ebo,
                vao,

                default_white_texture: create_default_white_texture(gl_api)
                    .context("could not create default white tex")?,
                textures: NoHashMap::default(),
            })
        }
    }

    // TODO: figure out how to invoke this xd.
    pub fn deinit(self, gl_api: &gl::wrap::Api) {
        unsafe {
            gl_api.delete_program(self.shader_rgba8.program);
            gl_api.delete_program(self.shader_r8.program);

            gl_api.delete_buffer(self.vbo);
            gl_api.delete_buffer(self.ebo);

            gl_api.delete_texture(self.default_white_texture.gl_handle);
            for (_, texture) in self.textures.iter() {
                gl_api.delete_texture(texture.gl_handle);
            }
        }
    }

    fn get_texture(&self, handle: sx::TextureHandle) -> &Texture {
        self.textures
            .get(&handle)
            .unwrap_or_else(|| panic!("invalid handle: {handle:?}"))
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
                    let texture = self.get_texture(command.handle);
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

    pub fn render<'a, E: sx::Externs + 'a>(
        &mut self,
        logical_size: sx::Vec2,
        scale_factor: f32,
        draw_data: impl Iterator<Item = &'a sx::DrawData<E>>,
        gl_api: &gl::wrap::Api,
    ) -> anyhow::Result<()>
    where
        E: sx::Externs<TextureHandle = <Self as sx::Externs>::TextureHandle>,
    {
        let physical_size = logical_size * scale_factor;
        let projection_matrix = compute_orthographic_projection_matrix(
            0.0,
            logical_size.x,
            logical_size.y,
            0.0,
            -1.0,
            1.0,
        );

        unsafe {
            // NOTE: draw buffer needs to be specififed.
            //   without i don't see anything being rendered on nvidia gpu,
            //   but on amd gpu it's fine.
            //
            // TODO: might want to try creating own frame buffer?
            gl_api.draw_buffer(gl::BACK);
            gl_api.viewport(
                0,
                0,
                physical_size.x as gl::GLsizei,
                physical_size.y as gl::GLsizei,
            );

            gl_api.enable(gl::BLEND);
            // TODO: do i need this func_add?
            gl_api.blend_equation(gl::FUNC_ADD);
            gl_api.blend_func_separate(
                gl::SRC_ALPHA,
                gl::ONE_MINUS_SRC_ALPHA,
                gl::ONE,
                gl::ONE_MINUS_SRC_ALPHA,
            );
        }

        unsafe {
            gl_api.bind_buffer(gl::ARRAY_BUFFER, Some(self.vbo));
            gl_api.bind_buffer(gl::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
            gl_api.bind_vertex_array(Some(self.vao));
        }

        for it in draw_data {
            unsafe {
                // TODO: should probably do buffer_sub_data here?
                gl_api.buffer_data(
                    gl::ARRAY_BUFFER,
                    (it.vertices.len() * size_of::<sx::Vertex>()) as gl::GLsizeiptr,
                    it.vertices.as_ptr().cast(),
                    gl::STREAM_DRAW,
                );
                gl_api.buffer_data(
                    gl::ELEMENT_ARRAY_BUFFER,
                    (it.indices.len() * size_of::<u32>()) as gl::GLsizeiptr,
                    it.indices.as_ptr().cast(),
                    gl::STREAM_DRAW,
                );
            }

            for sx::DrawCommand {
                clip_rect,
                index_range,
                texture,
            } in it.commands.iter()
            {
                if let Some(clip_rect) = clip_rect {
                    let physical_clip_rect = clip_rect.scale(scale_factor);
                    let x = physical_clip_rect.min.x as i32;
                    let y = physical_size.y as i32 - physical_clip_rect.max.y as i32;
                    let w = physical_clip_rect.width() as i32;
                    let h = physical_clip_rect.height() as i32;

                    unsafe {
                        gl_api.enable(gl::SCISSOR_TEST);
                        gl_api.scissor(x, y, w, h);
                    };
                }

                let (texture_gl_handle, texture_format) = texture.as_ref().map_or_else(
                    || {
                        let t = &self.default_white_texture;
                        (t.gl_handle, t.format)
                    },
                    |tex_kind| match tex_kind {
                        sx::TextureHandleKind::Internal(handle) => {
                            let t = self.get_texture(*handle);
                            (t.gl_handle, t.format)
                        }
                        sx::TextureHandleKind::External { handle, format } => (*handle, *format),
                    },
                );

                let shader = match texture_format {
                    TextureFormat::Rgba8Unorm => &self.shader_rgba8,
                    TextureFormat::R8Unorm => &self.shader_r8,
                };
                if self.active_program != Some(shader.program) {
                    self.active_program = Some(shader.program);

                    unsafe {
                        gl_api.use_program(Some(shader.program));

                        // TODO: is there such thing as uniform buffer objects (ubo)?
                        gl_api.uniform_matrix_4fv(
                            shader.u_projection_loc,
                            1,
                            gl::FALSE,
                            projection_matrix.as_ptr().cast(),
                        );
                    }
                }

                unsafe {
                    gl_api.active_texture(gl::TEXTURE0);
                    gl_api.bind_texture(gl::TEXTURE_2D, Some(texture_gl_handle));
                    gl_api.uniform_1i(shader.u_sampler_loc, 0);
                };

                unsafe {
                    gl_api.draw_elements(
                        gl::TRIANGLES,
                        (index_range.end - index_range.start) as gl::GLsizei,
                        gl::UNSIGNED_INT,
                        (index_range.start * size_of::<u32>() as u32) as *const c_void,
                    )
                };

                if clip_rect.is_some() {
                    unsafe { gl_api.disable(gl::SCISSOR_TEST) };
                }
            }
        }

        // NOTE: unset current shader to make sure that state for the next iteration will be
        // up-to-date.
        self.active_program = None;

        Ok(())
    }
}
