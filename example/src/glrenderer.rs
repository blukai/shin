use std::ffi::c_void;
use std::mem::offset_of;
use std::ptr::null;

use anyhow::{Context as _, anyhow};
use gl::Apier as _;
use nohash::NoHashMap;

const SHADER_SOURCE: &str = include_str!("shader.glsl");

unsafe fn create_shader(
    gl_api: &gl::Api,
    source: &str,
    r#type: gl::GLenum,
) -> anyhow::Result<gl::Shader> {
    unsafe {
        let shader = gl_api.create_shader(r#type)?;
        gl_api.shader_source(shader, source);
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
    gl_api: &gl::Api,
    vert_src: &str,
    frag_src: &str,
) -> anyhow::Result<gl::Program> {
    unsafe {
        let vert_shader = create_shader(gl_api, vert_src, gl::VERTEX_SHADER)?;
        let frag_shader = create_shader(gl_api, frag_src, gl::FRAGMENT_SHADER)?;

        let program = gl_api.create_program()?;

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

unsafe fn create_default_white_texture(gl_api: &gl::Api) -> anyhow::Result<gl::Texture> {
    unsafe {
        let texture = gl_api.create_texture()?;
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
            [255_u8; 4].as_ptr() as *const c_void,
        );

        Ok(texture)
    }
}

#[derive(Debug)]
pub struct GlRenderer {
    program: gl::Program,

    i_position_location: gl::GLint,
    i_tex_coord_location: gl::GLint,
    i_color_location: gl::GLint,
    u_view_size_location: gl::GLint,

    vbo: gl::Buffer,
    ebo: gl::Buffer,

    default_white_texture: gl::Texture,
    textures: NoHashMap<sx::TextureHandle, gl::Texture>,
}

impl sx::Externs for GlRenderer {
    type TextureHandle = gl::Texture;
}

impl GlRenderer {
    pub fn new(gl_api: &gl::Api) -> anyhow::Result<Self> {
        unsafe {
            let program = create_program(
                gl_api,
                &format!("#define VERTEX\n{SHADER_SOURCE}"),
                &format!("#define FRAGMENT\n{SHADER_SOURCE}"),
            )
            .context("could not create program")?;

            Ok(Self {
                program,

                i_position_location: gl_api
                    .get_attrib_location(program, c"i_position")
                    .context("could not get location of `i_position`")?,
                i_tex_coord_location: gl_api
                    .get_attrib_location(program, c"i_tex_coord")
                    .context("could not get location of `i_tex_coord`")?,
                i_color_location: gl_api
                    .get_attrib_location(program, c"i_color")
                    .context("could not get location of `i_color`")?,

                u_view_size_location: gl_api
                    .get_uniform_location(program, c"u_view_size")
                    .context("could not get location of `u_view_size`")?,

                vbo: gl_api.create_buffer().context("could not create vbo")?,
                ebo: gl_api.create_buffer().context("could not create ebo")?,

                default_white_texture: create_default_white_texture(gl_api)
                    .context("could not create default white tex")?,
                textures: NoHashMap::default(),
            })
        }
    }

    // TODO: figure out how to invoke this xd.
    pub fn destroy(self, gl_api: &gl::Api) {
        unsafe {
            gl_api.delete_program(self.program);

            gl_api.delete_buffer(self.vbo);
            gl_api.delete_buffer(self.ebo);

            gl_api.delete_texture(self.default_white_texture);
            for (_, texture) in self.textures.iter() {
                gl_api.delete_texture(*texture);
            }
        }
    }

    fn setup_state(&self, gl_api: &gl::Api) {
        unsafe {
            gl_api.use_program(Some(self.program));

            gl_api.enable(gl::BLEND);
            gl_api.blend_equation(gl::FUNC_ADD);
            gl_api.blend_func_separate(
                gl::SRC_ALPHA,
                gl::ONE_MINUS_SRC_ALPHA,
                gl::ONE,
                gl::ONE_MINUS_SRC_ALPHA,
            );

            // vertex
            gl_api.bind_buffer(gl::ARRAY_BUFFER, Some(self.vbo));
            gl_api.enable_vertex_attrib_array(self.i_position_location as gl::GLuint);
            gl_api.vertex_attrib_pointer(
                self.i_position_location as gl::GLuint,
                2,
                gl::FLOAT,
                gl::FALSE,
                size_of::<sx::Vertex>() as gl::GLsizei,
                offset_of!(sx::Vertex, position) as *const c_void,
            );
            gl_api.enable_vertex_attrib_array(self.i_tex_coord_location as gl::GLuint);
            gl_api.vertex_attrib_pointer(
                self.i_tex_coord_location as gl::GLuint,
                2,
                gl::FLOAT,
                gl::FALSE,
                size_of::<sx::Vertex>() as gl::GLsizei,
                offset_of!(sx::Vertex, tex_coord) as *const c_void,
            );
            gl_api.enable_vertex_attrib_array(self.i_color_location as gl::GLuint);
            gl_api.vertex_attrib_pointer(
                self.i_color_location as gl::GLuint,
                4,
                gl::UNSIGNED_BYTE,
                gl::FALSE,
                size_of::<sx::Vertex>() as gl::GLsizei,
                offset_of!(sx::Vertex, color) as *const c_void,
            );

            // index
            gl_api.bind_buffer(gl::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
        }
    }

    fn get_texture(&self, handle: sx::TextureHandle) -> gl::Texture {
        *self
            .textures
            .get(&handle)
            .unwrap_or_else(|| panic!("invalid handle: {handle:?}"))
    }

    pub fn handle_texture_commands<'a>(
        &mut self,
        texture_commands: impl Iterator<Item = sx::TextureCommand<&'a sx::TextureDesc, &'a [u8]>>,
        gl_api: &gl::Api,
    ) -> anyhow::Result<()> {
        for command in texture_commands {
            match command.kind {
                sx::TextureCommandKind::Create { desc } => {
                    assert!(!self.textures.contains_key(&command.handle));
                    let texture = unsafe {
                        let texture = gl_api
                            .create_texture()
                            .context("could not create texture")?;

                        gl_api.bind_texture(gl::TEXTURE_2D, Some(texture));

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

                        // NOTE: this fixes tilting when rendering bitmaps. see
                        // https://stackoverflow.com/questions/15983607/opengl-texture-tilted.
                        gl_api.pixel_storei(gl::UNPACK_ALIGNMENT, 1);

                        // TODO: describe_texture_format thing

                        // NOTE: this makes so that in the shader colors look like rgba 0 0 0 red,
                        // instead of just red. see
                        // https://www.khronos.org/opengl/wiki/Texture#Swizzle_mask
                        gl_api.tex_parameteriv(
                            gl::TEXTURE_2D,
                            gl::TEXTURE_SWIZZLE_RGBA,
                            [
                                gl::ONE as gl::GLint,
                                gl::ONE as gl::GLint,
                                gl::ONE as gl::GLint,
                                gl::RED as gl::GLint,
                            ]
                            .as_ptr(),
                        );

                        gl_api.tex_image_2d(
                            gl::TEXTURE_2D,
                            0,
                            gl::R8 as gl::GLint,
                            desc.w as gl::GLint,
                            desc.h as gl::GLint,
                            0,
                            gl::RED,
                            gl::UNSIGNED_BYTE,
                            null(),
                        );

                        texture
                    };
                    self.textures.insert(command.handle, texture);
                }
                sx::TextureCommandKind::Upload { region, buf } => {
                    let texture = self.get_texture(command.handle);
                    unsafe {
                        gl_api.bind_texture(gl::TEXTURE_2D, Some(texture));
                        // TODO: describe_texture_format thing
                        gl_api.tex_sub_image_2d(
                            gl::TEXTURE_2D,
                            0,
                            region.x as gl::GLint,
                            region.y as gl::GLint,
                            region.w as gl::GLsizei,
                            region.h as gl::GLsizei,
                            gl::RED,
                            gl::UNSIGNED_BYTE,
                            buf.as_ptr() as *const c_void,
                        );
                    }
                }
                sx::TextureCommandKind::Delete => {
                    let texture = self.get_texture(command.handle);
                    unsafe { gl_api.delete_texture(texture) };
                }
            }
        }
        Ok(())
    }

    pub fn render<E>(
        &mut self,
        logical_size: sx::Vec2,
        scale_factor: f32,
        draw_buffer: &sx::DrawBuffer<E>,
        gl_api: &gl::Api,
    ) -> anyhow::Result<()>
    where
        E: sx::Externs<TextureHandle = <Self as sx::Externs>::TextureHandle>,
    {
        let physical_size = logical_size * scale_factor;

        self.setup_state(gl_api);

        unsafe {
            gl_api.viewport(
                0,
                0,
                physical_size.x as gl::GLsizei,
                physical_size.y as gl::GLsizei,
            );
            gl_api.uniform_2f(
                self.u_view_size_location,
                logical_size.x as gl::GLfloat,
                logical_size.y as gl::GLfloat,
            );
        }

        for draw_data in draw_buffer.iter_draw_data() {
            unsafe {
                gl_api.buffer_data(
                    gl::ARRAY_BUFFER,
                    (draw_data.vertices.len() * size_of::<sx::Vertex>()) as gl::GLsizeiptr,
                    draw_data.vertices.as_ptr() as *const c_void,
                    gl::STREAM_DRAW,
                );
                gl_api.buffer_data(
                    gl::ELEMENT_ARRAY_BUFFER,
                    (draw_data.indices.len() * size_of::<u32>()) as gl::GLsizeiptr,
                    draw_data.indices.as_ptr() as *const c_void,
                    gl::STREAM_DRAW,
                );

                for sx::DrawCommand {
                    clip_rect,
                    index_range,
                    texture,
                } in draw_data.commands.iter()
                {
                    if let Some(clip_rect) = clip_rect {
                        gl_api.enable(gl::SCISSOR_TEST);

                        let physical_clip_rect = clip_rect.scale(scale_factor);
                        let x = physical_clip_rect.min.x as i32;
                        let y = physical_size.y as i32 - physical_clip_rect.max.y as i32;
                        let w = physical_clip_rect.width() as i32;
                        let h = physical_clip_rect.height() as i32;
                        gl_api.scissor(x, y, w, h);
                    }

                    gl_api.active_texture(gl::TEXTURE0);
                    gl_api.bind_texture(
                        gl::TEXTURE_2D,
                        Some(texture.as_ref().map_or_else(
                            || self.default_white_texture,
                            |tex_kind| match tex_kind {
                                sx::TextureHandleKind::Internal(handle) => {
                                    self.get_texture(*handle)
                                }
                                sx::TextureHandleKind::External(texture) => *texture,
                            },
                        )),
                    );

                    gl_api.draw_elements(
                        gl::TRIANGLES,
                        (index_range.end - index_range.start) as gl::GLsizei,
                        gl::UNSIGNED_INT,
                        (index_range.start * size_of::<u32>() as u32) as *const c_void,
                    );

                    if clip_rect.is_some() {
                        gl_api.disable(gl::SCISSOR_TEST);
                    }
                }
            }
        }

        Ok(())
    }
}
