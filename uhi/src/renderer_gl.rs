use std::{ffi::c_void, mem::offset_of, ptr::null};

use anyhow::{Context as _, anyhow};
use gpu::gl::{self, GlContexter};

use crate::{Context, Externs, TextureKind, TextureService, Vertex};

use super::Renderer;

const SHADER_SOURCE: &str = include_str!("shader.glsl");

unsafe fn create_shader(
    gl: &gl::Context,
    source: &str,
    r#type: gl::GLenum,
) -> anyhow::Result<gl::Shader> {
    unsafe {
        let shader = gl.create_shader(r#type)?;
        gl.shader_source(shader, source);
        gl.compile_shader(shader);

        let compile_status = gl.get_shader_parameter(shader, gl::COMPILE_STATUS);
        if compile_status == gl::FALSE as gl::GLint {
            let info_log = gl.get_shader_info_log(shader);
            Err(anyhow!("could not create shader: {info_log}"))
        } else {
            Ok(shader)
        }
    }
}

unsafe fn create_program(
    gl: &gl::Context,
    vert_src: &str,
    frag_src: &str,
) -> anyhow::Result<gl::Program> {
    unsafe {
        let vert_shader = create_shader(gl, vert_src, gl::VERTEX_SHADER)?;
        let frag_shader = create_shader(gl, frag_src, gl::FRAGMENT_SHADER)?;

        let program = gl.create_program()?;

        gl.attach_shader(program, vert_shader);
        gl.attach_shader(program, frag_shader);

        gl.link_program(program);

        gl.detach_shader(program, vert_shader);
        gl.detach_shader(program, frag_shader);

        gl.delete_shader(vert_shader);
        gl.delete_shader(frag_shader);

        let link_status = gl.get_program_parameter(program, gl::LINK_STATUS);
        if link_status == gl::FALSE as gl::GLint {
            let info_log = gl.get_program_info_log(program);
            Err(anyhow!("could not create program: {info_log}"))
        } else {
            Ok(program)
        }
    }
}

unsafe fn create_default_white_tex(gl: &gl::Context) -> anyhow::Result<gl::Texture> {
    unsafe {
        let texture = gl.create_texture()?;
        gl.bind_texture(gl::TEXTURE_2D, Some(texture));

        gl.tex_image_2d(
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

    default_white_tex: gl::Texture,
}

impl GlRenderer {
    pub fn new(gl: &gl::Context) -> anyhow::Result<Self> {
        unsafe {
            let program = create_program(
                gl,
                &format!("#define VERTEX\n{SHADER_SOURCE}"),
                &format!("#define FRAGMENT\n{SHADER_SOURCE}"),
            )
            .context("could not create program")?;

            Ok(Self {
                program,

                i_position_location: gl
                    .get_attrib_location(program, c"i_position")
                    .context("could not get location of `i_position`")?,
                i_tex_coord_location: gl
                    .get_attrib_location(program, c"i_tex_coord")
                    .context("could not get location of `i_tex_coord`")?,
                i_color_location: gl
                    .get_attrib_location(program, c"i_color")
                    .context("could not get location of `i_color`")?,

                u_view_size_location: gl
                    .get_uniform_location(program, c"u_view_size")
                    .context("could not get location of `u_view_size`")?,

                vbo: gl.create_buffer().context("could not create vbo")?,
                ebo: gl.create_buffer().context("could not create ebo")?,

                default_white_tex: create_default_white_tex(gl)
                    .context("could not create default white tex")?,
            })
        }
    }

    fn setup_state(&self, gl: &gl::Context) {
        unsafe {
            gl.use_program(Some(self.program));

            gl.enable(gl::BLEND);
            gl.blend_equation(gl::FUNC_ADD);
            gl.blend_func_separate(
                gl::SRC_ALPHA,
                gl::ONE_MINUS_SRC_ALPHA,
                gl::ONE,
                gl::ONE_MINUS_SRC_ALPHA,
            );

            // vertex
            gl.bind_buffer(gl::ARRAY_BUFFER, Some(self.vbo));
            gl.enable_vertex_attrib_array(self.i_position_location as gl::GLuint);
            gl.vertex_attrib_pointer(
                self.i_position_location as gl::GLuint,
                2,
                gl::FLOAT,
                gl::FALSE,
                size_of::<Vertex>() as gl::GLsizei,
                offset_of!(Vertex, position) as *const c_void,
            );
            gl.enable_vertex_attrib_array(self.i_tex_coord_location as gl::GLuint);
            gl.vertex_attrib_pointer(
                self.i_tex_coord_location as gl::GLuint,
                2,
                gl::FLOAT,
                gl::FALSE,
                size_of::<Vertex>() as gl::GLsizei,
                offset_of!(Vertex, tex_coord) as *const c_void,
            );
            gl.enable_vertex_attrib_array(self.i_color_location as gl::GLuint);
            gl.vertex_attrib_pointer(
                self.i_color_location as gl::GLuint,
                4,
                gl::UNSIGNED_BYTE,
                gl::FALSE,
                size_of::<Vertex>() as gl::GLsizei,
                offset_of!(Vertex, color) as *const c_void,
            );

            // index
            gl.bind_buffer(gl::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
        }
    }

    fn handle_textures<E>(
        &self,
        gl: &gl::Context,
        texture_service: &mut TextureService<E>,
    ) -> anyhow::Result<()>
    where
        E: Externs<TextureHandle = <Self as Renderer>::TextureHandle>,
    {
        while let Some(texture) = texture_service.next_pending_destroy() {
            unsafe { gl.delete_texture(texture) };
        }

        while let Some((ticket, desc)) = texture_service.next_pending_create() {
            let texture = unsafe {
                let texture = gl
                    .create_texture()
                    .context("could not create ftc page tex")?;

                gl.bind_texture(gl::TEXTURE_2D, Some(texture));

                // NOTE: without those params you can't see shit in this mist
                //
                // NOTE: to deal with min and mag filters, etc. - you might want to
                // consider introducing SamplerDescriptor and TextureViewDescriptor
                gl.tex_parameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
                gl.tex_parameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);

                // NOTE: this fixes tilting when rendering bitmaps. see
                // https://stackoverflow.com/questions/15983607/opengl-texture-tilted.
                gl.pixel_storei(gl::UNPACK_ALIGNMENT, 1);

                // TODO: describe_texture_format thing

                // NOTE: this makes so that in the shader colors look like rgba 0 0 0 red,
                // instead of just red. see
                // https://www.khronos.org/opengl/wiki/Texture#Swizzle_mask
                gl.tex_parameteriv(
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

                gl.tex_image_2d(
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
            texture_service.commit_create(ticket, texture);
        }

        while let Some(update) = texture_service.next_pending_update() {
            unsafe {
                gl.bind_texture(gl::TEXTURE_2D, Some(*update.texture));
                // TODO: describe_texture_format thing
                gl.tex_sub_image_2d(
                    gl::TEXTURE_2D,
                    0,
                    update.region.x as gl::GLint,
                    update.region.y as gl::GLint,
                    update.region.w as gl::GLsizei,
                    update.region.h as gl::GLsizei,
                    gl::RED,
                    gl::UNSIGNED_BYTE,
                    update.data.as_ptr() as *const c_void,
                );
            }
        }

        Ok(())
    }

    pub fn render<E>(
        &self,
        ctx: &mut Context<E>,
        gl: &gl::Context,
        view_size: (u32, u32),
    ) -> anyhow::Result<()>
    where
        E: Externs<TextureHandle = <Self as Renderer>::TextureHandle>,
    {
        self.setup_state(gl);
        self.handle_textures(gl, &mut ctx.texture_service)?;

        let draw_data = ctx.get_draw_data();

        unsafe {
            gl.viewport(0, 0, view_size.0 as gl::GLsizei, view_size.1 as gl::GLsizei);

            gl.uniform_2f(
                self.u_view_size_location,
                view_size.0 as gl::GLfloat,
                view_size.1 as gl::GLfloat,
            );

            gl.buffer_data(
                gl::ARRAY_BUFFER,
                (draw_data.vertices.len() * size_of::<Vertex>()) as gl::GLsizeiptr,
                draw_data.vertices.as_ptr() as *const c_void,
                gl::STREAM_DRAW,
            );
            gl.buffer_data(
                gl::ELEMENT_ARRAY_BUFFER,
                (draw_data.indices.len() * size_of::<u32>()) as gl::GLsizeiptr,
                draw_data.indices.as_ptr() as *const c_void,
                gl::STREAM_DRAW,
            );

            for draw_command in draw_data.commands.iter() {
                gl.active_texture(gl::TEXTURE0);
                gl.bind_texture(
                    gl::TEXTURE_2D,
                    Some(draw_command.texture.as_ref().map_or_else(
                        || self.default_white_tex,
                        |tex_kind| match tex_kind {
                            TextureKind::Internal(internal) => *ctx.texture_service.get(*internal),
                            TextureKind::External(external) => *external,
                        },
                    )),
                );

                gl.draw_elements(
                    gl::TRIANGLES,
                    (draw_command.index_range.end - draw_command.index_range.start) as gl::GLsizei,
                    gl::UNSIGNED_INT,
                    (draw_command.index_range.start * size_of::<u32>() as u32) as *const c_void,
                );
            }
        }

        Ok(())
    }
}

impl Renderer for GlRenderer {
    type TextureHandle = gl::Texture;
}
