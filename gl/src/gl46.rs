use std::ffi::{CStr, c_char, c_void};
use std::num::NonZero;

use anyhow::{Context as _, anyhow};

use super::Adapter;
use super::enums::*;
use super::types::*;

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
mod api {
    use crate::types::*;

    include!(concat!(env!("OUT_DIR"), "/gl_api_generated.rs"));
}

pub struct Api {
    api: api::Api,
}

impl Api {
    pub unsafe fn load_with<F>(get_proc_address: F) -> Self
    where
        F: FnMut(*const std::ffi::c_char) -> *mut std::ffi::c_void,
    {
        Self {
            api: unsafe { api::Api::load_with(get_proc_address) },
        }
    }
}

impl Adapter for Api {
    type Buffer = NonZero<GLuint>;
    type Program = NonZero<GLuint>;
    type Shader = NonZero<GLuint>;
    type Texture = NonZero<GLuint>;

    #[inline]
    unsafe fn active_texture(&self, texture: GLenum) {
        unsafe { self.api.ActiveTexture(texture) };
    }

    #[inline]
    unsafe fn attach_shader(&self, program: Self::Program, shader: Self::Shader) {
        unsafe { self.api.AttachShader(program.get(), shader.get()) };
    }

    #[inline]
    unsafe fn bind_attrib_location(&self, program: Self::Program, index: GLuint, name: &CStr) {
        unsafe {
            self.api
                .BindAttribLocation(program.get(), index, name.as_ptr())
        };
    }

    #[inline]
    unsafe fn bind_buffer(&self, target: GLenum, buffer: Option<Self::Buffer>) {
        unsafe {
            self.api
                .BindBuffer(target, buffer.map_or_else(|| 0, |v| v.get()))
        };
    }

    #[inline]
    unsafe fn bind_texture(&self, target: GLenum, texture: Option<Self::Texture>) {
        unsafe {
            self.api
                .BindTexture(target, texture.map_or_else(|| 0, |v| v.get()))
        };
    }

    #[inline]
    unsafe fn blend_equation(&self, mode: GLenum) {
        unsafe { self.api.BlendEquation(mode) };
    }

    #[inline]
    unsafe fn blend_func_separate(
        &self,
        src_rgb: GLenum,
        dst_rgb: GLenum,
        src_alpha: GLenum,
        dst_alpha: GLenum,
    ) {
        unsafe {
            self.api
                .BlendFuncSeparate(src_rgb, dst_rgb, src_alpha, dst_alpha)
        };
    }

    #[inline]
    unsafe fn buffer_data(
        &self,
        target: GLenum,
        size: GLsizeiptr,
        data: *const c_void,
        usage: GLenum,
    ) {
        unsafe { self.api.BufferData(target, size, data, usage) };
    }

    #[inline]
    unsafe fn clear(&self, mask: GLbitfield) {
        unsafe { self.api.Clear(mask) };
    }

    #[inline]
    unsafe fn clear_color(&self, red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) {
        unsafe { self.api.ClearColor(red, green, blue, alpha) };
    }

    #[inline]
    unsafe fn compile_shader(&self, shader: Self::Shader) {
        unsafe { self.api.CompileShader(shader.get()) };
    }

    #[inline]
    unsafe fn create_buffer(&self) -> anyhow::Result<Self::Buffer> {
        let mut buffer: GLuint = 0;
        unsafe { self.api.GenBuffers(1, &mut buffer) };
        NonZero::new(buffer).context("could not create buffer")
    }

    #[inline]
    unsafe fn create_program(&self) -> anyhow::Result<Self::Program> {
        let program = unsafe { self.api.CreateProgram() };
        NonZero::new(program).context("could not create program")
    }

    #[inline]
    unsafe fn create_shader(&self, r#type: GLenum) -> anyhow::Result<Self::Shader> {
        let program = unsafe { self.api.CreateShader(r#type) };
        NonZero::new(program).context("could not create shader")
    }

    #[inline]
    unsafe fn create_texture(&self) -> anyhow::Result<Self::Texture> {
        let mut texture: GLuint = 0;
        unsafe { self.api.GenTextures(1, &mut texture) };
        NonZero::new(texture).context("could not create texture")
    }

    #[inline]
    unsafe fn delete_buffer(&self, buffer: Self::Buffer) {
        unsafe { self.api.DeleteBuffers(1, &buffer.get()) };
    }

    #[inline]
    unsafe fn delete_program(&self, program: Self::Program) {
        unsafe { self.api.DeleteProgram(program.get()) };
    }

    #[inline]
    unsafe fn delete_shader(&self, shader: Self::Shader) {
        unsafe { self.api.DeleteShader(shader.get()) };
    }

    #[inline]
    unsafe fn delete_texture(&self, texture: Self::Texture) {
        unsafe { self.api.DeleteTextures(1, &texture.get()) };
    }

    #[inline]
    unsafe fn detach_shader(&self, program: Self::Program, shader: Self::Shader) {
        unsafe { self.api.DetachShader(program.get(), shader.get()) };
    }

    #[inline]
    unsafe fn disable(&self, cap: GLenum) {
        unsafe { self.api.Disable(cap) };
    }

    #[inline]
    unsafe fn draw_elements(
        &self,
        mode: GLenum,
        count: GLsizei,
        r#type: GLenum,
        indices: *const c_void,
    ) {
        unsafe { self.api.DrawElements(mode, count, r#type, indices) }
    }

    #[inline]
    unsafe fn enable(&self, cap: GLenum) {
        unsafe { self.api.Enable(cap) };
    }

    #[inline]
    unsafe fn enable_vertex_attrib_array(&self, index: GLuint) {
        unsafe { self.api.EnableVertexAttribArray(index) };
    }

    #[inline]
    unsafe fn get_attrib_location(&self, program: Self::Program, name: &CStr) -> Option<GLint> {
        let ret = unsafe { self.api.GetAttribLocation(program.get(), name.as_ptr()) };
        (ret != -1).then_some(ret)
    }

    #[inline]
    unsafe fn get_error(&self) -> Option<GLenum> {
        let ret = unsafe { self.api.GetError() };
        (ret != NO_ERROR).then_some(ret)
    }

    #[inline]
    unsafe fn get_program_info_log(&self, program: Self::Program) -> String {
        let mut len = unsafe { self.get_shader_parameter(program, INFO_LOG_LENGTH) };
        let mut info_log = vec![0; len as usize];
        unsafe {
            self.api.GetProgramInfoLog(
                program.get(),
                len,
                &mut len,
                info_log.as_mut_ptr() as *mut GLchar,
            );
            String::from_utf8_unchecked(info_log)
        }
    }

    #[inline]
    unsafe fn get_program_parameter(&self, program: Self::Program, pname: GLenum) -> GLint {
        let mut param: GLint = 0;
        unsafe { self.api.GetProgramiv(program.get(), pname, &mut param) };
        param
    }

    #[inline]
    unsafe fn get_shader_info_log(&self, shader: Self::Shader) -> String {
        let mut len = unsafe { self.get_shader_parameter(shader, INFO_LOG_LENGTH) };
        let mut info_log = vec![0; len as usize];
        unsafe {
            self.api.GetShaderInfoLog(
                shader.get(),
                len,
                &mut len,
                info_log.as_mut_ptr() as *mut GLchar,
            );
            String::from_utf8_unchecked(info_log)
        }
    }

    #[inline]
    unsafe fn get_shader_parameter(&self, shader: Self::Shader, pname: GLenum) -> GLint {
        let mut param: GLint = 0;
        unsafe { self.api.GetShaderiv(shader.get(), pname, &mut param) };
        param
    }

    #[inline]
    unsafe fn get_string(&self, name: GLenum) -> anyhow::Result<String> {
        let ptr = unsafe { self.api.GetString(name) };
        if ptr.is_null() {
            return Err(anyhow!("could not get string (name 0x{name:x})"));
        }
        unsafe {
            CStr::from_ptr(ptr as *const c_char)
                .to_str()
                .context("invalid string")
                .map(|cstr| cstr.to_string())
        }
    }

    #[inline]
    unsafe fn get_uniform_location(&self, program: Self::Program, name: &CStr) -> Option<GLint> {
        let ret = unsafe { self.api.GetUniformLocation(program.get(), name.as_ptr()) };
        (ret != -1).then_some(ret)
    }

    #[inline]
    unsafe fn link_program(&self, program: Self::Program) {
        unsafe { self.api.LinkProgram(program.get()) };
    }

    #[inline]
    unsafe fn pixel_storei(&self, pname: GLenum, param: GLint) {
        unsafe { self.api.PixelStorei(pname, param) };
    }

    #[inline]
    unsafe fn read_pixels(
        &self,
        x: GLint,
        y: GLint,
        width: GLsizei,
        height: GLsizei,
        format: GLenum,
        r#type: GLenum,
        pixels: *mut c_void,
    ) {
        unsafe {
            self.api
                .ReadPixels(x, y, width, height, format, r#type, pixels)
        };
    }

    #[inline]
    unsafe fn scissor(&self, x: GLint, y: GLint, width: GLsizei, height: GLsizei) {
        unsafe { self.api.Scissor(x, y, width, height) };
    }

    #[inline]
    unsafe fn shader_source(&self, shader: Self::Shader, source: &str) {
        unsafe {
            self.api.ShaderSource(
                shader.get(),
                1,
                &(source.as_ptr() as *const GLchar),
                &(source.len() as GLint),
            )
        };
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
        unsafe {
            self.api.TexImage2D(
                target,
                level,
                internalformat,
                width,
                height,
                border,
                format,
                r#type,
                pixels,
            )
        };
    }

    #[inline]
    unsafe fn tex_parameteri(&self, target: GLenum, pname: GLenum, param: GLint) {
        unsafe { self.api.TexParameteri(target, pname, param) };
    }

    #[inline]
    unsafe fn tex_parameteriv(&self, target: GLenum, pname: GLenum, params: *const GLint) {
        unsafe { self.api.TexParameteriv(target, pname, params) };
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
        unsafe {
            self.api.TexSubImage2D(
                target, level, xoffset, yoffset, width, height, format, r#type, pixels,
            )
        };
    }

    #[inline]
    unsafe fn uniform_1f(&self, location: GLint, v0: GLfloat) {
        unsafe { self.api.Uniform1f(location, v0) };
    }

    #[inline]
    unsafe fn uniform_1i(&self, location: GLint, v0: GLint) {
        unsafe { self.api.Uniform1i(location, v0) };
    }

    #[inline]
    unsafe fn uniform_2f(&self, location: GLint, v0: GLfloat, v1: GLfloat) {
        unsafe { self.api.Uniform2f(location, v0, v1) };
    }

    #[inline]
    unsafe fn use_program(&self, program: Option<Self::Program>) {
        unsafe { self.api.UseProgram(program.map_or_else(|| 0, |v| v.get())) };
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
        unsafe {
            self.api
                .VertexAttribPointer(index, size, r#type, normalized, stride, pointer)
        };
    }

    #[inline]
    unsafe fn viewport(&self, x: GLint, y: GLint, width: GLsizei, height: GLsizei) {
        unsafe { self.api.Viewport(x, y, width, height) };
    }
}
