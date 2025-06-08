use std::ffi::{CStr, c_void};

use super::GlContext;
use super::types::*;

unsafe extern "C" {
    fn gl_clear_color(extern_ref: u32, red: f32, green: f32, blue: f32, alpha: f32);
    fn gl_clear(extern_ref: u32, mask: u32);
}

pub struct Context {
    extern_ref: u32,
}

impl Context {
    pub fn from_extern_ref(extern_ref: u32) -> Self {
        Self { extern_ref }
    }
}

impl GlContext for Context {
    type Buffer = u32;
    type Program = u32;
    type Shader = u32;
    type Texture = u32;

    #[inline]
    unsafe fn active_texture(&self, texture: GLenum) {
        todo!()
    }

    #[inline]
    unsafe fn attach_shader(&self, program: Self::Program, shader: Self::Shader) {
        todo!()
    }

    #[inline]
    unsafe fn bind_attrib_location(&self, program: Self::Program, index: GLuint, name: &CStr) {
        todo!()
    }

    #[inline]
    unsafe fn bind_buffer(&self, target: GLenum, buffer: Option<Self::Buffer>) {
        todo!()
    }

    #[inline]
    unsafe fn bind_texture(&self, target: GLenum, texture: Option<Self::Texture>) {
        todo!()
    }

    #[inline]
    unsafe fn blend_equation(&self, mode: GLenum) {
        todo!()
    }

    #[inline]
    unsafe fn blend_func_separate(
        &self,
        src_rgb: GLenum,
        dst_rgb: GLenum,
        src_alpha: GLenum,
        dst_alpha: GLenum,
    ) {
        todo!()
    }

    #[inline]
    unsafe fn buffer_data(
        &self,
        target: GLenum,
        size: GLsizeiptr,
        data: *const c_void,
        usage: GLenum,
    ) {
        todo!()
    }

    #[inline]
    unsafe fn clear(&self, mask: GLbitfield) {
        unsafe { gl_clear(self.extern_ref, mask) }
    }

    #[inline]
    unsafe fn clear_color(&self, red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) {
        unsafe { gl_clear_color(self.extern_ref, red, green, blue, alpha) }
    }

    #[inline]
    unsafe fn compile_shader(&self, shader: Self::Shader) {
        todo!()
    }

    #[inline]
    unsafe fn create_buffer(&self) -> anyhow::Result<Self::Buffer> {
        todo!()
    }

    #[inline]
    unsafe fn create_program(&self) -> anyhow::Result<Self::Program> {
        todo!()
    }

    #[inline]
    unsafe fn create_shader(&self, r#type: GLenum) -> anyhow::Result<Self::Shader> {
        todo!()
    }

    #[inline]
    unsafe fn create_texture(&self) -> anyhow::Result<Self::Texture> {
        todo!()
    }

    #[inline]
    unsafe fn delete_buffer(&self, buffer: Self::Buffer) {
        todo!()
    }

    #[inline]
    unsafe fn delete_program(&self, program: Self::Program) {
        todo!()
    }

    #[inline]
    unsafe fn delete_shader(&self, shader: Self::Shader) {
        todo!()
    }

    #[inline]
    unsafe fn delete_texture(&self, texture: Self::Texture) {
        todo!()
    }

    #[inline]
    unsafe fn detach_shader(&self, program: Self::Program, shader: Self::Shader) {
        todo!()
    }

    #[inline]
    unsafe fn draw_elements(
        &self,
        mode: GLenum,
        count: GLsizei,
        r#type: GLenum,
        indices: *const c_void,
    ) {
        todo!()
    }

    #[inline]
    unsafe fn enable(&self, cap: GLenum) {
        todo!()
    }

    #[inline]
    unsafe fn enable_vertex_attrib_array(&self, index: GLuint) {
        todo!()
    }

    #[inline]
    unsafe fn get_attrib_location(&self, program: Self::Program, name: &CStr) -> Option<GLint> {
        todo!()
    }

    #[inline]
    unsafe fn get_error(&self) -> Option<GLenum> {
        todo!()
    }

    #[inline]
    unsafe fn get_program_info_log(&self, program: Self::Program) -> String {
        todo!()
    }

    #[inline]
    unsafe fn get_program_parameter(&self, program: Self::Program, pname: GLenum) -> GLint {
        todo!()
    }

    #[inline]
    unsafe fn get_shader_info_log(&self, shader: Self::Shader) -> String {
        todo!()
    }

    #[inline]
    unsafe fn get_shader_parameter(&self, shader: Self::Shader, pname: GLenum) -> GLint {
        todo!()
    }

    #[inline]
    unsafe fn get_string(&self, name: GLenum) -> anyhow::Result<String> {
        todo!()
    }

    #[inline]
    unsafe fn get_uniform_location(&self, program: Self::Program, name: &CStr) -> Option<GLint> {
        todo!()
    }

    #[inline]
    unsafe fn link_program(&self, program: Self::Program) {
        todo!()
    }

    #[inline]
    unsafe fn pixel_storei(&self, pname: GLenum, param: GLint) {
        todo!()
    }

    #[inline]
    unsafe fn shader_source(&self, shader: Self::Shader, source: &str) {
        todo!()
    }

    #[inline]
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
    ) {
        todo!()
    }

    #[inline]
    unsafe fn tex_parameteri(&self, target: GLenum, pname: GLenum, param: GLint) {
        todo!()
    }

    #[inline]
    unsafe fn tex_parameteriv(&self, target: GLenum, pname: GLenum, params: *const GLint) {
        todo!()
    }

    #[inline]
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
    ) {
        todo!()
    }

    #[inline]
    unsafe fn uniform_1f(&self, location: GLint, v0: GLfloat) {
        todo!()
    }

    #[inline]
    unsafe fn uniform_1i(&self, location: GLint, v0: GLint) {
        todo!()
    }

    #[inline]
    unsafe fn uniform_2f(&self, location: GLint, v0: GLfloat, v1: GLfloat) {
        todo!()
    }

    #[inline]
    unsafe fn use_program(&self, program: Option<Self::Program>) {
        todo!()
    }

    #[inline]
    unsafe fn vertex_attrib_pointer(
        &self,
        index: GLuint,
        size: GLint,
        r#type: GLenum,
        normalized: GLboolean,
        stride: GLsizei,
        pointer: *const c_void,
    ) {
        todo!()
    }

    #[inline]
    unsafe fn viewport(&self, x: GLint, y: GLint, width: GLsizei, height: GLsizei) {
        todo!()
    }
}
