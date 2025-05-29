use std::{ffi::c_void, mem::offset_of, ptr::null};

use anyhow::{Context as _, anyhow};
use gpu::gl::{self, GlContexter};

use crate::{TextureKind, TextureService, Vertex, drawbuffer::DrawBuffer};

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

    fn handle_textures(
        &self,
        ctx: &gl::Context,
        texture_service: &mut TextureService<Self>,
    ) -> anyhow::Result<()> {
        while let Some((ticket, desc)) = texture_service.next_pending_create() {
            let texture = unsafe {
                let texture = ctx
                    .create_texture()
                    .context("could not create ftc page tex")?;

                ctx.bind_texture(gl::TEXTURE_2D, Some(texture));

                // NOTE: without those params you can't see shit in this mist
                //
                // NOTE: to deal with min and mag filters, etc. - you might want to
                // consider introducing SamplerDescriptor and TextureViewDescriptor
                ctx.tex_parameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
                ctx.tex_parameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);

                // NOTE: this fixes tilting when rendering bitmaps. see
                // https://stackoverflow.com/questions/15983607/opengl-texture-tilted.
                ctx.pixel_storei(gl::UNPACK_ALIGNMENT, 1);

                // TODO: describe_texture_format thing

                // NOTE: this makes so that in the shader colors look like rgba 0 0 0 red,
                // instead of just red. see
                // https://www.khronos.org/opengl/wiki/Texture#Swizzle_mask
                ctx.tex_parameteriv(
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

                ctx.tex_image_2d(
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

        while let Some((ticket, update)) = texture_service.next_pending_update() {
            let texture = texture_service.get(update.handle);
            unsafe {
                ctx.bind_texture(gl::TEXTURE_2D, Some(*texture));
                // TODO: describe_texture_format thing
                ctx.tex_sub_image_2d(
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
            texture_service.commit_update(ticket);
        }

        Ok(())
    }

    pub fn render(
        &self,
        ctx: &gl::Context,
        size: (u32, u32),
        // TODO: make DrawBuffer also mut; it needs to drainable?
        draw_buffer: &DrawBuffer<Self>,
        texture_service: &mut TextureService<Self>,
    ) -> anyhow::Result<()> {
        self.setup_state(ctx);
        self.handle_textures(ctx, texture_service)?;

        unsafe {
            ctx.viewport(0, 0, size.0 as gl::GLsizei, size.1 as gl::GLsizei);

            ctx.uniform_2f(
                self.u_view_size_location,
                size.0 as gl::GLfloat,
                size.1 as gl::GLfloat,
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
                    Some(draw_command.tex_kind.as_ref().map_or_else(
                        || self.default_white_tex,
                        |tex_kind| match tex_kind {
                            TextureKind::Internal(internal) => *texture_service.get(*internal),
                            TextureKind::External(external) => *external,
                        },
                    )),
                );

                ctx.draw_elements(
                    gl::TRIANGLES,
                    (draw_command.end_index - draw_command.start_index) as gl::GLsizei,
                    gl::UNSIGNED_INT,
                    (draw_command.start_index * size_of::<u32>() as u32) as *const c_void,
                );
            }
        }

        Ok(())
    }
}

impl Renderer for GlRenderer {
    type TextureHandle = gl::Texture;
}
