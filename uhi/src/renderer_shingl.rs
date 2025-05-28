use std::{ffi::c_void, mem::offset_of};

use anyhow::{Context, anyhow};
use gpu::gl::{self, GlContexter};

use crate::{Vertex, drawbuffer::DrawBuffer};

use super::Renderer;

const SHADER_SOURCE: &str = include_str!("shader.glsl");

unsafe fn create_shader(
    ctx: &gl::Context,
    source: &str,
    r#type: gl::GLenum,
) -> anyhow::Result<gl::Shader> {
    unsafe {
        let shader = ctx.create_shader(r#type)?;
        ctx.shader_source(shader, source);
        ctx.compile_shader(shader);

        let compile_status = ctx.get_shader_parameter(shader, gl::COMPILE_STATUS);
        if compile_status == gl::FALSE as gl::GLint {
            let info_log = ctx.get_shader_info_log(shader);
            Err(anyhow!("could not create shader: {info_log}"))
        } else {
            Ok(shader)
        }
    }
}

unsafe fn create_program(
    ctx: &gl::Context,
    vert_src: &str,
    frag_src: &str,
) -> anyhow::Result<gl::Program> {
    unsafe {
        let vert_shader = create_shader(ctx, vert_src, gl::VERTEX_SHADER)?;
        let frag_shader = create_shader(ctx, frag_src, gl::FRAGMENT_SHADER)?;

        let program = ctx.create_program()?;

        ctx.attach_shader(program, vert_shader);
        ctx.attach_shader(program, frag_shader);

        ctx.link_program(program);

        ctx.detach_shader(program, vert_shader);
        ctx.detach_shader(program, frag_shader);

        ctx.delete_shader(vert_shader);
        ctx.delete_shader(frag_shader);

        let link_status = ctx.get_program_parameter(program, gl::LINK_STATUS);
        if link_status == gl::FALSE as gl::GLint {
            let info_log = ctx.get_program_info_log(program);
            Err(anyhow!("could not create program: {info_log}"))
        } else {
            Ok(program)
        }
    }
}

unsafe fn create_default_white_tex(ctx: &gl::Context) -> anyhow::Result<gl::Texture> {
    unsafe {
        let texture = ctx.create_texture()?;
        ctx.bind_texture(gl::TEXTURE_2D, Some(texture));

        ctx.tex_image_2d(
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
    pub fn new(ctx: &gl::Context) -> anyhow::Result<Self> {
        unsafe {
            let program = create_program(
                ctx,
                &format!("#define VERTEX\n{SHADER_SOURCE}"),
                &format!("#define FRAGMENT\n{SHADER_SOURCE}"),
            )
            .context("could not create program")?;

            Ok(Self {
                program,

                i_position_location: ctx
                    .get_attrib_location(program, c"i_position")
                    .context("could not get location of `i_position`")?,
                i_tex_coord_location: ctx
                    .get_attrib_location(program, c"i_tex_coord")
                    .context("could not get location of `i_tex_coord`")?,
                i_color_location: ctx
                    .get_attrib_location(program, c"i_color")
                    .context("could not get location of `i_color`")?,

                u_view_size_location: ctx
                    .get_uniform_location(program, c"u_view_size")
                    .context("could not get location of `u_view_size`")?,

                vbo: ctx.create_buffer().context("could not create vbo")?,
                ebo: ctx.create_buffer().context("could not create ebo")?,

                default_white_tex: create_default_white_tex(ctx)
                    .context("could not create default white tex")?,
            })
        }
    }

    fn setup_state(&self, ctx: &gl::Context) {
        unsafe {
            ctx.use_program(Some(self.program));

            ctx.enable(gl::BLEND);
            ctx.blend_equation(gl::FUNC_ADD);
            ctx.blend_func_separate(
                gl::SRC_ALPHA,
                gl::ONE_MINUS_SRC_ALPHA,
                gl::ONE,
                gl::ONE_MINUS_SRC_ALPHA,
            );

            // vertex
            ctx.bind_buffer(gl::ARRAY_BUFFER, Some(self.vbo));
            ctx.enable_vertex_attrib_array(self.i_position_location as gl::GLuint);
            ctx.vertex_attrib_pointer(
                self.i_position_location as gl::GLuint,
                2,
                gl::FLOAT,
                gl::FALSE,
                size_of::<Vertex>() as gl::GLsizei,
                offset_of!(Vertex, position) as *const c_void,
            );
            ctx.enable_vertex_attrib_array(self.i_tex_coord_location as gl::GLuint);
            ctx.vertex_attrib_pointer(
                self.i_tex_coord_location as gl::GLuint,
                2,
                gl::FLOAT,
                gl::FALSE,
                size_of::<Vertex>() as gl::GLsizei,
                offset_of!(Vertex, tex_coord) as *const c_void,
            );
            ctx.enable_vertex_attrib_array(self.i_color_location as gl::GLuint);
            ctx.vertex_attrib_pointer(
                self.i_color_location as gl::GLuint,
                4,
                gl::UNSIGNED_BYTE,
                gl::FALSE,
                size_of::<Vertex>() as gl::GLsizei,
                offset_of!(Vertex, color) as *const c_void,
            );

            // index
            ctx.bind_buffer(gl::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
        }
    }

    pub fn render(
        &self,
        ctx: &gl::Context,
        logical_size: (u32, u32),
        draw_buffer: &DrawBuffer<Self>,
    ) {
        unsafe {
            self.setup_state(ctx);

            ctx.uniform_2f(
                self.u_view_size_location,
                logical_size.0 as gl::GLfloat,
                logical_size.1 as gl::GLfloat,
            );

            ctx.buffer_data(
                gl::ARRAY_BUFFER,
                (draw_buffer.vertices.len() * size_of::<Vertex>()) as gl::GLsizeiptr,
                draw_buffer.vertices.as_ptr() as *const c_void,
                gl::STREAM_DRAW,
            );
            ctx.buffer_data(
                gl::ELEMENT_ARRAY_BUFFER,
                (draw_buffer.indices.len() * size_of::<u32>()) as gl::GLsizeiptr,
                draw_buffer.indices.as_ptr() as *const c_void,
                gl::STREAM_DRAW,
            );

            for draw_command in draw_buffer.draw_commands.iter() {
                ctx.active_texture(gl::TEXTURE0);
                ctx.bind_texture(
                    gl::TEXTURE_2D,
                    Some(draw_command.tex_handle.unwrap_or(self.default_white_tex)),
                );

                ctx.draw_elements(
                    gl::TRIANGLES,
                    (draw_command.end_index - draw_command.start_index) as gl::GLsizei,
                    gl::UNSIGNED_INT,
                    (draw_command.start_index * size_of::<u32>() as u32) as *const c_void,
                );
            }
        }
    }
}

impl Renderer for GlRenderer {
    type TextureHandle = gl::Texture;
}
