use std::{ffi::c_void, mem::offset_of, ptr::null};

use anyhow::{Context as _, anyhow};
use gl::api::Apier as _;

use crate::{Context, DrawCommand, Externs, TextureKind, TextureService, Vertex};

use super::Renderer;

const SHADER_SOURCE: &str = include_str!("shader.glsl");

unsafe fn create_shader(
    gl_api: &gl::api::Api,
    source: &str,
    r#type: gl::api::GLenum,
) -> anyhow::Result<gl::api::Shader> {
    unsafe {
        let shader = gl_api.create_shader(r#type)?;
        gl_api.shader_source(shader, source);
        gl_api.compile_shader(shader);

        let compile_status = gl_api.get_shader_parameter(shader, gl::api::COMPILE_STATUS);
        if compile_status == gl::api::FALSE as gl::api::GLint {
            let info_log = gl_api.get_shader_info_log(shader);
            Err(anyhow!("could not create shader: {info_log}"))
        } else {
            Ok(shader)
        }
    }
}

unsafe fn create_program(
    gl_api: &gl::api::Api,
    vert_src: &str,
    frag_src: &str,
) -> anyhow::Result<gl::api::Program> {
    unsafe {
        let vert_shader = create_shader(gl_api, vert_src, gl::api::VERTEX_SHADER)?;
        let frag_shader = create_shader(gl_api, frag_src, gl::api::FRAGMENT_SHADER)?;

        let program = gl_api.create_program()?;

        gl_api.attach_shader(program, vert_shader);
        gl_api.attach_shader(program, frag_shader);

        gl_api.link_program(program);

        gl_api.detach_shader(program, vert_shader);
        gl_api.detach_shader(program, frag_shader);

        gl_api.delete_shader(vert_shader);
        gl_api.delete_shader(frag_shader);

        let link_status = gl_api.get_program_parameter(program, gl::api::LINK_STATUS);
        if link_status == gl::api::FALSE as gl::api::GLint {
            let info_log = gl_api.get_program_info_log(program);
            Err(anyhow!("could not create program: {info_log}"))
        } else {
            Ok(program)
        }
    }
}

unsafe fn create_default_white_tex(gl_api: &gl::api::Api) -> anyhow::Result<gl::api::Texture> {
    unsafe {
        let texture = gl_api.create_texture()?;
        gl_api.bind_texture(gl::api::TEXTURE_2D, Some(texture));

        gl_api.tex_image_2d(
            gl::api::TEXTURE_2D,
            0,
            gl::api::RGBA8 as gl::api::GLint,
            1,
            1,
            0,
            gl::api::RGBA,
            gl::api::UNSIGNED_BYTE,
            [255_u8; 4].as_ptr() as *const c_void,
        );

        Ok(texture)
    }
}

#[derive(Debug)]
pub struct GlRenderer {
    program: gl::api::Program,

    i_position_location: gl::api::GLint,
    i_tex_coord_location: gl::api::GLint,
    i_color_location: gl::api::GLint,
    u_view_size_location: gl::api::GLint,

    vbo: gl::api::Buffer,
    ebo: gl::api::Buffer,

    default_white_tex: gl::api::Texture,
}

impl GlRenderer {
    pub fn new(gl_api: &gl::api::Api) -> anyhow::Result<Self> {
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

                default_white_tex: create_default_white_tex(gl_api)
                    .context("could not create default white tex")?,
            })
        }
    }

    fn setup_state(&self, gl_api: &gl::api::Api) {
        unsafe {
            gl_api.use_program(Some(self.program));

            gl_api.enable(gl::api::BLEND);
            gl_api.blend_equation(gl::api::FUNC_ADD);
            gl_api.blend_func_separate(
                gl::api::SRC_ALPHA,
                gl::api::ONE_MINUS_SRC_ALPHA,
                gl::api::ONE,
                gl::api::ONE_MINUS_SRC_ALPHA,
            );

            // vertex
            gl_api.bind_buffer(gl::api::ARRAY_BUFFER, Some(self.vbo));
            gl_api.enable_vertex_attrib_array(self.i_position_location as gl::api::GLuint);
            gl_api.vertex_attrib_pointer(
                self.i_position_location as gl::api::GLuint,
                2,
                gl::api::FLOAT,
                gl::api::FALSE,
                size_of::<Vertex>() as gl::api::GLsizei,
                offset_of!(Vertex, position) as *const c_void,
            );
            gl_api.enable_vertex_attrib_array(self.i_tex_coord_location as gl::api::GLuint);
            gl_api.vertex_attrib_pointer(
                self.i_tex_coord_location as gl::api::GLuint,
                2,
                gl::api::FLOAT,
                gl::api::FALSE,
                size_of::<Vertex>() as gl::api::GLsizei,
                offset_of!(Vertex, tex_coord) as *const c_void,
            );
            gl_api.enable_vertex_attrib_array(self.i_color_location as gl::api::GLuint);
            gl_api.vertex_attrib_pointer(
                self.i_color_location as gl::api::GLuint,
                4,
                gl::api::UNSIGNED_BYTE,
                gl::api::FALSE,
                size_of::<Vertex>() as gl::api::GLsizei,
                offset_of!(Vertex, color) as *const c_void,
            );

            // index
            gl_api.bind_buffer(gl::api::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
        }
    }

    fn handle_textures<E>(
        &self,
        gl_api: &gl::api::Api,
        texture_service: &mut TextureService<E>,
    ) -> anyhow::Result<()>
    where
        E: Externs<TextureHandle = <Self as Renderer>::TextureHandle>,
    {
        while let Some(texture) = texture_service.next_pending_destroy() {
            unsafe { gl_api.delete_texture(texture) };
        }

        while let Some((ticket, desc)) = texture_service.next_pending_create() {
            let texture = unsafe {
                let texture = gl_api
                    .create_texture()
                    .context("could not create ftc page tex")?;

                gl_api.bind_texture(gl::api::TEXTURE_2D, Some(texture));

                // NOTE: without those params you can't see shit in this mist
                //
                // NOTE: to deal with min and mag filters, etc. - you might want to
                // consider introducing SamplerDescriptor and TextureViewDescriptor
                gl_api.tex_parameteri(
                    gl::api::TEXTURE_2D,
                    gl::api::TEXTURE_MIN_FILTER,
                    gl::api::NEAREST as _,
                );
                gl_api.tex_parameteri(
                    gl::api::TEXTURE_2D,
                    gl::api::TEXTURE_MAG_FILTER,
                    gl::api::NEAREST as _,
                );

                // NOTE: this fixes tilting when rendering bitmaps. see
                // https://stackoverflow.com/questions/15983607/opengl-texture-tilted.
                gl_api.pixel_storei(gl::api::UNPACK_ALIGNMENT, 1);

                // TODO: describe_texture_format thing

                // NOTE: this makes so that in the shader colors look like rgba 0 0 0 red,
                // instead of just red. see
                // https://www.khronos.org/opengl/wiki/Texture#Swizzle_mask
                gl_api.tex_parameteriv(
                    gl::api::TEXTURE_2D,
                    gl::api::TEXTURE_SWIZZLE_RGBA,
                    [
                        gl::api::ONE as gl::api::GLint,
                        gl::api::ONE as gl::api::GLint,
                        gl::api::ONE as gl::api::GLint,
                        gl::api::RED as gl::api::GLint,
                    ]
                    .as_ptr(),
                );

                gl_api.tex_image_2d(
                    gl::api::TEXTURE_2D,
                    0,
                    gl::api::R8 as gl::api::GLint,
                    desc.w as gl::api::GLint,
                    desc.h as gl::api::GLint,
                    0,
                    gl::api::RED,
                    gl::api::UNSIGNED_BYTE,
                    null(),
                );

                texture
            };
            texture_service.commit_create(ticket, texture);
        }

        while let Some(update) = texture_service.next_pending_update() {
            unsafe {
                gl_api.bind_texture(gl::api::TEXTURE_2D, Some(*update.texture));
                // TODO: describe_texture_format thing
                gl_api.tex_sub_image_2d(
                    gl::api::TEXTURE_2D,
                    0,
                    update.region.x as gl::api::GLint,
                    update.region.y as gl::api::GLint,
                    update.region.w as gl::api::GLsizei,
                    update.region.h as gl::api::GLsizei,
                    gl::api::RED,
                    gl::api::UNSIGNED_BYTE,
                    update.data.as_ptr() as *const c_void,
                );
            }
        }

        Ok(())
    }

    pub fn render<E>(
        &self,
        ctx: &mut Context<E>,
        gl_api: &gl::api::Api,
        view_size: (u32, u32),
    ) -> anyhow::Result<()>
    where
        E: Externs<TextureHandle = <Self as Renderer>::TextureHandle>,
    {
        self.setup_state(gl_api);
        self.handle_textures(gl_api, &mut ctx.texture_service)?;

        let draw_data = ctx.draw_buffer.get_draw_data();

        unsafe {
            gl_api.viewport(
                0,
                0,
                view_size.0 as gl::api::GLsizei,
                view_size.1 as gl::api::GLsizei,
            );

            gl_api.uniform_2f(
                self.u_view_size_location,
                view_size.0 as gl::api::GLfloat,
                view_size.1 as gl::api::GLfloat,
            );

            gl_api.buffer_data(
                gl::api::ARRAY_BUFFER,
                (draw_data.vertices.len() * size_of::<Vertex>()) as gl::api::GLsizeiptr,
                draw_data.vertices.as_ptr() as *const c_void,
                gl::api::STREAM_DRAW,
            );
            gl_api.buffer_data(
                gl::api::ELEMENT_ARRAY_BUFFER,
                (draw_data.indices.len() * size_of::<u32>()) as gl::api::GLsizeiptr,
                draw_data.indices.as_ptr() as *const c_void,
                gl::api::STREAM_DRAW,
            );

            for DrawCommand {
                // TODO: make use of clip rect (apply scissor).
                clip_rect,
                index_range,
                texture,
            } in draw_data.commands.iter()
            {
                gl_api.active_texture(gl::api::TEXTURE0);
                gl_api.bind_texture(
                    gl::api::TEXTURE_2D,
                    Some(texture.as_ref().map_or_else(
                        || self.default_white_tex,
                        |tex_kind| match tex_kind {
                            TextureKind::Internal(internal) => *ctx.texture_service.get(*internal),
                            TextureKind::External(external) => *external,
                        },
                    )),
                );

                gl_api.draw_elements(
                    gl::api::TRIANGLES,
                    (index_range.end - index_range.start) as gl::api::GLsizei,
                    gl::api::UNSIGNED_INT,
                    (index_range.start * size_of::<u32>() as u32) as *const c_void,
                );
            }
        }

        Ok(())
    }
}

impl Renderer for GlRenderer {
    type TextureHandle = gl::api::Texture;
}
