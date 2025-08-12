use std::ffi::{CStr, c_void};

pub(crate) mod types {
    include!(concat!(env!("OUT_DIR"), "/gl_types_generated.rs"));
}

#[allow(non_upper_case_globals)]
pub(crate) mod enums {
    use super::types::*;

    include!(concat!(env!("OUT_DIR"), "/gl_enums_generated.rs"));
}

#[cfg(not(target_family = "wasm"))]
#[path = "api_gl46.rs"]
mod api_gl46;

#[cfg(target_family = "wasm")]
#[path = "api_webgl2.rs"]
mod api_webgl2;

#[cfg(not(target_family = "wasm"))]
pub use api_gl46::*;
#[cfg(target_family = "wasm")]
pub use api_webgl2::*;
pub use enums::*;
pub use types::*;

pub trait Apier {
    type Buffer;
    type Program;
    type Shader;
    type Texture;

    unsafe fn active_texture(&self, texture: GLenum);
    unsafe fn attach_shader(&self, program: Self::Program, shader: Self::Shader);
    unsafe fn bind_attrib_location(&self, program: Self::Program, index: GLuint, name: &CStr);
    unsafe fn bind_buffer(&self, target: GLenum, buffer: Option<Self::Buffer>);
    unsafe fn bind_texture(&self, target: GLenum, texture: Option<Self::Texture>);
    unsafe fn blend_equation(&self, mode: GLenum);
    unsafe fn blend_func_separate(
        &self,
        src_rgb: GLenum,
        dst_rgb: GLenum,
        src_alpha: GLenum,
        dst_alpha: GLenum,
    );
    unsafe fn buffer_data(
        &self,
        target: GLenum,
        size: GLsizeiptr,
        data: *const c_void,
        usage: GLenum,
    );
    unsafe fn clear(&self, mask: GLbitfield);
    unsafe fn clear_color(&self, red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat);
    unsafe fn compile_shader(&self, shader: Self::Shader);
    unsafe fn create_buffer(&self) -> anyhow::Result<Self::Buffer>;
    unsafe fn create_program(&self) -> anyhow::Result<Self::Program>;
    unsafe fn create_shader(&self, r#type: GLenum) -> anyhow::Result<Self::Shader>;
    unsafe fn create_texture(&self) -> anyhow::Result<Self::Texture>;
    unsafe fn delete_buffer(&self, buffer: Self::Buffer);
    unsafe fn delete_program(&self, program: Self::Program);
    unsafe fn delete_shader(&self, shader: Self::Shader);
    unsafe fn delete_texture(&self, texture: Self::Texture);
    unsafe fn detach_shader(&self, program: Self::Program, shader: Self::Shader);
    unsafe fn disable(&self, cap: GLenum);
    unsafe fn draw_elements(
        &self,
        mode: GLenum,
        count: GLsizei,
        r#type: GLenum,
        indices: *const c_void,
    );
    unsafe fn enable(&self, cap: GLenum);
    unsafe fn enable_vertex_attrib_array(&self, index: GLuint);
    unsafe fn get_attrib_location(&self, program: Self::Program, name: &CStr) -> Option<GLint>;
    unsafe fn get_error(&self) -> Option<GLenum>;
    unsafe fn get_program_info_log(&self, program: Self::Program) -> String;
    unsafe fn get_program_parameter(&self, program: Self::Program, pname: GLenum) -> GLint;
    unsafe fn get_shader_info_log(&self, shader: Self::Shader) -> String;
    unsafe fn get_shader_parameter(&self, shader: Self::Shader, pname: GLenum) -> GLint;
    unsafe fn get_string(&self, name: GLenum) -> anyhow::Result<String>;
    unsafe fn get_uniform_location(&self, program: Self::Program, name: &CStr) -> Option<GLint>;
    unsafe fn link_program(&self, program: Self::Program);
    unsafe fn pixel_storei(&self, pname: GLenum, param: GLint);
    unsafe fn scissor(&self, x: GLint, y: GLint, width: GLsizei, height: GLsizei);
    unsafe fn shader_source(&self, shader: Self::Shader, source: &str);
    unsafe fn tex_image_2d(
        &self,
        target: GLenum,
        level: GLint,
        internalformat: GLint,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
        format: GLenum,
        r#type: GLenum,
        pixels: *const c_void,
    );
    unsafe fn tex_parameteri(&self, target: GLenum, pname: GLenum, param: GLint);
    unsafe fn tex_parameteriv(&self, target: GLenum, pname: GLenum, params: *const GLint);
    unsafe fn tex_sub_image_2d(
        &self,
        target: GLenum,
        level: GLint,
        xoffset: GLint,
        yoffset: GLint,
        width: GLsizei,
        height: GLsizei,
        format: GLenum,
        r#type: GLenum,
        pixels: *const c_void,
    );
    unsafe fn uniform_1f(&self, location: GLint, v0: GLfloat);
    unsafe fn uniform_1i(&self, location: GLint, v0: GLint);
    unsafe fn uniform_2f(&self, location: GLint, v0: GLfloat, v1: GLfloat);
    unsafe fn use_program(&self, program: Option<Self::Program>);
    unsafe fn vertex_attrib_pointer(
        &self,
        index: GLuint,
        size: GLint,
        r#type: GLenum,
        normalized: GLboolean,
        stride: GLsizei,
        pointer: *const c_void,
    );
    unsafe fn viewport(&self, x: GLint, y: GLint, width: GLsizei, height: GLsizei);
}

pub type Buffer = <Api as Apier>::Buffer;
pub type Program = <Api as Apier>::Program;
pub type Shader = <Api as Apier>::Shader;
pub type Texture = <Api as Apier>::Texture;
