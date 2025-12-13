use std::ffi::{CStr, c_void};

use crate::libgl as gl;

// NOTE: why not just use glow?
// i very don't like that its Api trait does not exactly mirror gl spec.
// i don't like that it abstracts away certain thigs, i don't want that.
//
// TODO: make sure that all methods match libgl's methods 1:1 with the exception of things that can
// be rustified (like strings and stuff).
//
// NOTE: rustfmt is disabled for this trait because i absofuckinglutely hate that it breaks (in
// this specific case) long methods into multiple lines. it's just inconvenient.
#[rustfmt::skip]
pub trait Adapter {
    type Buffer: Clone;
    type Framebuffer: Clone;
    type Program: Clone;
    type Renderbuffer: Clone;
    type Shader: Clone;
    type Texture: Clone;
    type UniformLocation: Clone;
    type VertexArray: Clone;

    unsafe fn active_texture(&self, texture: gl::GLenum);
    unsafe fn attach_shader(&self, program: Self::Program, shader: Self::Shader);
    unsafe fn bind_attrib_location(&self, program: Self::Program, index: gl::GLuint, name: &CStr);
    unsafe fn bind_buffer(&self, target: gl::GLenum, buffer: Option<Self::Buffer>);
    unsafe fn bind_framebuffer(&self, target: gl::GLenum, framebuffer: Option<Self::Framebuffer>);
    unsafe fn bind_renderbuffer(&self, target: gl::GLenum, renderbuffer: Option<Self::Renderbuffer>);
    unsafe fn bind_texture(&self, target: gl::GLenum, texture: Option<Self::Texture>);
    unsafe fn bind_vertex_array(&self, array: Option<Self::VertexArray>);
    unsafe fn blend_equation(&self, mode: gl::GLenum);
    unsafe fn blend_func_separate(&self, sfactor_rgb: gl::GLenum, dfactor_rgb: gl::GLenum, sfactor_alpha: gl::GLenum, dfactor_alpha: gl::GLenum);
    unsafe fn blit_framebuffer(&self, src_x0: gl::GLint, src_y0: gl::GLint, src_x1: gl::GLint, src_y1: gl::GLint, dst_x0: gl::GLint, dst_y0: gl::GLint, dst_x1: gl::GLint, dst_y1: gl::GLint, mask: gl::GLbitfield, filter: gl::GLenum);
    // TODO: buffer_data can be rustified.
    unsafe fn buffer_data(&self, target: gl::GLenum, size: gl::GLsizeiptr, data: *const c_void, usage: gl::GLenum);
    unsafe fn check_framebuffer_status(&self, target: gl::GLenum) -> gl::GLenum;
    unsafe fn clear(&self, mask: gl::GLbitfield);
    unsafe fn clear_color(&self, red: gl::GLfloat, green: gl::GLfloat, blue: gl::GLfloat, alpha: gl::GLfloat);
    unsafe fn compile_shader(&self, shader: Self::Shader);
    unsafe fn create_buffer(&self) -> Option<Self::Buffer>;
    unsafe fn create_framebuffer(&self) -> Option<Self::Framebuffer>;
    unsafe fn create_program(&self) -> Option<Self::Program>;
    unsafe fn create_renderbuffer(&self) -> Option<Self::Renderbuffer>;
    unsafe fn create_shader(&self, r#type: gl::GLenum) -> Option<Self::Shader>;
    unsafe fn create_texture(&self) -> Option<Self::Texture>;
    unsafe fn create_vertex_array(&self) -> Option<Self::VertexArray>;
    unsafe fn delete_buffer(&self, buffer: Self::Buffer);
    unsafe fn delete_framebuffer(&self, framebuffer: Self::Framebuffer);
    unsafe fn delete_program(&self, program: Self::Program);
    unsafe fn delete_renderbuffer(&self, renderbuffer: Self::Renderbuffer);
    unsafe fn delete_shader(&self, shader: Self::Shader);
    unsafe fn delete_texture(&self, texture: Self::Texture);
    unsafe fn detach_shader(&self, program: Self::Program, shader: Self::Shader);
    unsafe fn disable(&self, cap: gl::GLenum);
    unsafe fn draw_buffer(&self, buf: gl::GLenum);
    unsafe fn draw_elements(&self, mode: gl::GLenum, count: gl::GLsizei, r#type: gl::GLenum, indices: *const c_void);
    unsafe fn enable(&self, cap: gl::GLenum);
    unsafe fn enable_vertex_attrib_array(&self, index: gl::GLuint);
    unsafe fn framebuffer_renderbuffer(&self, target: gl::GLenum, attachment: gl::GLenum, renderbuffertarget: gl::GLenum, renderbuffer: Option<Self::Renderbuffer>);
    unsafe fn framebuffer_texture_2d(&self, target: gl::GLenum, attachment: gl::GLenum, textarget: gl::GLenum, texture: Option<Self::Texture>, level: gl::GLint);
    unsafe fn get_attrib_location(&self, program: Self::Program, name: &CStr) -> Option<gl::GLint>;
    unsafe fn get_error(&self) -> Option<gl::GLenum>;
    // TODO: consider making get_program_info_log to not allocate, but instead accept a resizable
    // buffer.
    unsafe fn get_program_info_log(&self, program: Self::Program) -> String;
    // TODO: same issue as with get_shader_parameter.
    unsafe fn get_program_parameter(&self, program: Self::Program, pname: gl::GLenum) -> gl::GLint;
    // TODO: consider making get_shader_info_log to not allocate, but instead accept a resizable
    // buffer.
    unsafe fn get_shader_info_log(&self, shader: Self::Shader) -> String;
    // TODO: why the fuck would you want to rename getshaderiv to get_shader_parameter? be
    // consistent!
    unsafe fn get_shader_parameter(&self, shader: Self::Shader, pname: gl::GLenum) -> gl::GLint;
    // TODO: docs say that glGetString returns pointer to a static string. why are we allocating? 
    unsafe fn get_string(&self, name: gl::GLenum) -> Option<String>;
    unsafe fn get_uniform_location(&self, program: Self::Program, name: &CStr) -> Option<Self::UniformLocation>;
    unsafe fn link_program(&self, program: Self::Program);
    unsafe fn pixel_storei(&self, pname: gl::GLenum, param: gl::GLint);
    unsafe fn read_buffer(&self, src: gl::GLenum);
    unsafe fn read_pixels(&self, x: gl::GLint, y: gl::GLint, width: gl::GLsizei, height: gl::GLsizei, format: gl::GLenum, r#type: gl::GLenum, pixels: *mut c_void);
    unsafe fn renderbuffer_storage(&self, target: gl::GLenum, internalformat: gl::GLenum, width: gl::GLsizei, height: gl::GLsizei);
    unsafe fn scissor(&self, x: gl::GLint, y: gl::GLint, width: gl::GLsizei, height: gl::GLsizei);
    unsafe fn shader_source(&self, shader: Self::Shader, source: &str);
    unsafe fn tex_image_2d(&self, target: gl::GLenum, level: gl::GLint, internalformat: gl::GLint, width: gl::GLsizei, height: gl::GLsizei, border: gl::GLint, format: gl::GLenum, r#type: gl::GLenum, pixels: *const c_void);
    unsafe fn tex_parameteri(&self, target: gl::GLenum, pname: gl::GLenum, param: gl::GLint);
    unsafe fn tex_parameteriv(&self, target: gl::GLenum, pname: gl::GLenum, params: *const gl::GLint);
    unsafe fn tex_sub_image_2d(&self, target: gl::GLenum, level: gl::GLint, xoffset: gl::GLint, yoffset: gl::GLint, width: gl::GLsizei, height: gl::GLsizei, format: gl::GLenum, r#type: gl::GLenum, pixels: *const c_void);
    unsafe fn uniform_1f(&self, location: Self::UniformLocation, v0: gl::GLfloat);
    unsafe fn uniform_1i(&self, location: Self::UniformLocation, v0: gl::GLint);
    unsafe fn uniform_2f(&self, location: Self::UniformLocation, v0: gl::GLfloat, v1: gl::GLfloat);
    unsafe fn uniform_4f(&self, location: Self::UniformLocation, v0: gl::GLfloat, v1: gl::GLfloat, v2: gl::GLfloat, v3: gl::GLfloat);
    unsafe fn uniform_matrix_4fv(&self, location: Self::UniformLocation, count: gl::GLsizei, transpose: gl::GLboolean, value: *const gl::GLfloat);
    unsafe fn use_program(&self, program: Option<Self::Program>);
    unsafe fn vertex_attrib_pointer(&self, index: gl::GLuint, size: gl::GLint, r#type: gl::GLenum, normalized: gl::GLboolean, stride: gl::GLsizei, pointer: *const c_void);
    unsafe fn viewport(&self, x: gl::GLint, y: gl::GLint, width: gl::GLsizei, height: gl::GLsizei);
}

#[cfg(not(target_family = "wasm"))]
mod gl46 {
    use std::ffi::{CStr, c_char, c_void};

    use super::Adapter;
    use crate::libgl as gl;

    pub struct Api {
        api: gl::Api,
    }

    impl Api {
        pub unsafe fn load_with<F>(get_proc_address: F) -> Self
        where
            F: FnMut(*const c_char) -> *mut std::ffi::c_void,
        {
            let api = unsafe { gl::Api::load_with(get_proc_address) };
            Self { api }
        }
    }

    impl Adapter for Api {
        type Buffer = gl::GLuint;
        type Framebuffer = gl::GLuint;
        type Program = gl::GLuint;
        type Renderbuffer = gl::GLuint;
        type Shader = gl::GLuint;
        type Texture = gl::GLuint;
        type UniformLocation = gl::GLuint;
        type VertexArray = gl::GLuint;

        #[inline]
        unsafe fn active_texture(&self, texture: gl::GLenum) {
            unsafe { self.api.ActiveTexture(texture) };
        }

        #[inline]
        unsafe fn attach_shader(&self, program: Self::Program, shader: Self::Shader) {
            unsafe { self.api.AttachShader(program, shader) };
        }

        #[inline]
        unsafe fn bind_attrib_location(
            &self,
            program: Self::Program,
            index: gl::GLuint,
            name: &CStr,
        ) {
            unsafe { self.api.BindAttribLocation(program, index, name.as_ptr()) };
        }

        #[inline]
        unsafe fn bind_buffer(&self, target: gl::GLenum, buffer: Option<Self::Buffer>) {
            unsafe { self.api.BindBuffer(target, buffer.unwrap_or(0)) };
        }

        #[inline]
        unsafe fn bind_framebuffer(
            &self,
            target: gl::GLenum,
            framebuffer: Option<Self::Framebuffer>,
        ) {
            unsafe { self.api.BindFramebuffer(target, framebuffer.unwrap_or(0)) };
        }

        #[inline]
        unsafe fn bind_renderbuffer(
            &self,
            target: gl::GLenum,
            renderbuffer: Option<Self::Renderbuffer>,
        ) {
            unsafe { self.api.BindRenderbuffer(target, renderbuffer.unwrap_or(0)) };
        }

        #[inline]
        unsafe fn bind_texture(&self, target: gl::GLenum, texture: Option<Self::Texture>) {
            unsafe { self.api.BindTexture(target, texture.unwrap_or(0)) };
        }

        #[inline]
        unsafe fn bind_vertex_array(&self, array: Option<Self::VertexArray>) {
            unsafe { self.api.BindVertexArray(array.unwrap_or(0)) };
        }

        #[inline]
        unsafe fn blend_equation(&self, mode: gl::GLenum) {
            unsafe { self.api.BlendEquation(mode) };
        }

        #[inline]
        unsafe fn blend_func_separate(
            &self,
            sfactor_rgb: gl::GLenum,
            dfactor_rgb: gl::GLenum,
            sfactor_alpha: gl::GLenum,
            dfactor_alpha: gl::GLenum,
        ) {
            unsafe {
                self.api
                    .BlendFuncSeparate(sfactor_rgb, dfactor_rgb, sfactor_alpha, dfactor_alpha)
            };
        }

        #[inline]
        unsafe fn check_framebuffer_status(&self, target: gl::GLenum) -> gl::GLenum {
            unsafe { self.api.CheckFramebufferStatus(target) }
        }

        #[inline]
        unsafe fn blit_framebuffer(
            &self,
            src_x0: gl::GLint,
            src_y0: gl::GLint,
            src_x1: gl::GLint,
            src_y1: gl::GLint,
            dst_x0: gl::GLint,
            dst_y0: gl::GLint,
            dst_x1: gl::GLint,
            dst_y1: gl::GLint,
            mask: gl::GLbitfield,
            filter: gl::GLenum,
        ) {
            unsafe {
                self.api.BlitFramebuffer(
                    src_x0, src_y0, src_x1, src_y1, dst_x0, dst_y0, dst_x1, dst_y1, mask, filter,
                )
            };
        }

        #[inline]
        unsafe fn buffer_data(
            &self,
            target: gl::GLenum,
            size: gl::GLsizeiptr,
            data: *const c_void,
            usage: gl::GLenum,
        ) {
            unsafe { self.api.BufferData(target, size, data, usage) };
        }

        #[inline]
        unsafe fn clear(&self, mask: gl::GLbitfield) {
            unsafe { self.api.Clear(mask) };
        }

        #[inline]
        unsafe fn clear_color(
            &self,
            red: gl::GLfloat,
            green: gl::GLfloat,
            blue: gl::GLfloat,
            alpha: gl::GLfloat,
        ) {
            unsafe { self.api.ClearColor(red, green, blue, alpha) };
        }

        #[inline]
        unsafe fn compile_shader(&self, shader: Self::Shader) {
            unsafe { self.api.CompileShader(shader) };
        }

        #[inline]
        unsafe fn create_buffer(&self) -> Option<Self::Buffer> {
            let mut buffer: gl::GLuint = 0;
            unsafe { self.api.GenBuffers(1, &mut buffer) };
            (buffer > 0).then_some(buffer)
        }

        #[inline]
        unsafe fn create_framebuffer(&self) -> Option<Self::Framebuffer> {
            let mut framebuffer: gl::GLuint = 0;
            unsafe { self.api.GenFramebuffers(1, &mut framebuffer) };
            (framebuffer > 0).then_some(framebuffer)
        }

        #[inline]
        unsafe fn create_program(&self) -> Option<Self::Program> {
            let program = unsafe { self.api.CreateProgram() };
            (program > 0).then_some(program)
        }

        #[inline]
        unsafe fn create_renderbuffer(&self) -> Option<Self::Renderbuffer> {
            let mut renderbuffer: gl::GLuint = 0;
            unsafe { self.api.GenRenderbuffers(1, &mut renderbuffer) };
            (renderbuffer > 0).then_some(renderbuffer)
        }

        #[inline]
        unsafe fn create_shader(&self, r#type: gl::GLenum) -> Option<Self::Shader> {
            let shader = unsafe { self.api.CreateShader(r#type) };
            (shader > 0).then_some(shader)
        }

        #[inline]
        unsafe fn create_texture(&self) -> Option<Self::Texture> {
            let mut texture: gl::GLuint = 0;
            unsafe { self.api.GenTextures(1, &mut texture) };
            (texture > 0).then_some(texture)
        }

        #[inline]
        unsafe fn create_vertex_array(&self) -> Option<Self::VertexArray> {
            let mut vertex_array: gl::GLuint = 0;
            unsafe { self.api.GenVertexArrays(1, &mut vertex_array) };
            (vertex_array > 0).then_some(vertex_array)
        }

        #[inline]
        unsafe fn delete_buffer(&self, buffer: Self::Buffer) {
            unsafe { self.api.DeleteBuffers(1, &buffer) };
        }

        #[inline]
        unsafe fn delete_framebuffer(&self, framebuffer: Self::Framebuffer) {
            unsafe { self.api.DeleteFramebuffers(1, &framebuffer) };
        }

        #[inline]
        unsafe fn delete_program(&self, program: Self::Program) {
            unsafe { self.api.DeleteProgram(program) };
        }

        #[inline]
        unsafe fn delete_renderbuffer(&self, renderbuffer: Self::Renderbuffer) {
            unsafe { self.api.DeleteRenderbuffers(1, &renderbuffer) };
        }

        #[inline]
        unsafe fn delete_shader(&self, shader: Self::Shader) {
            unsafe { self.api.DeleteShader(shader) };
        }

        #[inline]
        unsafe fn delete_texture(&self, texture: Self::Texture) {
            unsafe { self.api.DeleteTextures(1, &texture) };
        }

        #[inline]
        unsafe fn detach_shader(&self, program: Self::Program, shader: Self::Shader) {
            unsafe { self.api.DetachShader(program, shader) };
        }

        #[inline]
        unsafe fn disable(&self, cap: gl::GLenum) {
            unsafe { self.api.Disable(cap) };
        }

        #[inline]
        unsafe fn draw_buffer(&self, buf: gl::GLenum) {
            unsafe { self.api.DrawBuffer(buf) };
        }

        #[inline]
        unsafe fn draw_elements(
            &self,
            mode: gl::GLenum,
            count: gl::GLsizei,
            r#type: gl::GLenum,
            indices: *const c_void,
        ) {
            unsafe { self.api.DrawElements(mode, count, r#type, indices) }
        }

        #[inline]
        unsafe fn enable(&self, cap: gl::GLenum) {
            unsafe { self.api.Enable(cap) };
        }

        #[inline]
        unsafe fn enable_vertex_attrib_array(&self, index: gl::GLuint) {
            unsafe { self.api.EnableVertexAttribArray(index) };
        }

        #[inline]
        unsafe fn framebuffer_renderbuffer(
            &self,
            target: gl::GLenum,
            attachment: gl::GLenum,
            renderbuffertarget: gl::GLenum,
            renderbuffer: Option<Self::Renderbuffer>,
        ) {
            unsafe {
                self.api.FramebufferRenderbuffer(
                    target,
                    attachment,
                    renderbuffertarget,
                    renderbuffer.unwrap_or(0),
                )
            };
        }

        #[inline]
        unsafe fn framebuffer_texture_2d(
            &self,
            target: gl::GLenum,
            attachment: gl::GLenum,
            textarget: gl::GLenum,
            texture: Option<Self::Texture>,
            level: gl::GLint,
        ) {
            unsafe {
                self.api.FramebufferTexture2D(
                    target,
                    attachment,
                    textarget,
                    texture.unwrap_or(0),
                    level,
                )
            };
        }

        #[inline]
        unsafe fn get_attrib_location(
            &self,
            program: Self::Program,
            name: &CStr,
        ) -> Option<gl::GLint> {
            let ret = unsafe { self.api.GetAttribLocation(program, name.as_ptr()) };
            (ret != -1).then_some(ret)
        }

        #[inline]
        unsafe fn get_error(&self) -> Option<gl::GLenum> {
            let ret = unsafe { self.api.GetError() };
            (ret != gl::NO_ERROR).then_some(ret)
        }

        #[inline]
        unsafe fn get_program_info_log(&self, program: Self::Program) -> String {
            let mut len = unsafe { self.get_shader_parameter(program, gl::INFO_LOG_LENGTH) };
            let mut info_log = vec![0; len as usize];
            unsafe {
                self.api.GetProgramInfoLog(
                    program,
                    len,
                    &mut len,
                    info_log.as_mut_ptr() as *mut gl::GLchar,
                );
                String::from_utf8_unchecked(info_log)
            }
        }

        #[inline]
        unsafe fn get_program_parameter(
            &self,
            program: Self::Program,
            pname: gl::GLenum,
        ) -> gl::GLint {
            let mut param: gl::GLint = 0;
            unsafe { self.api.GetProgramiv(program, pname, &mut param) };
            param
        }

        #[inline]
        unsafe fn get_shader_info_log(&self, shader: Self::Shader) -> String {
            let mut len = unsafe { self.get_shader_parameter(shader, gl::INFO_LOG_LENGTH) };
            let mut info_log = vec![0; len as usize];
            unsafe {
                self.api.GetShaderInfoLog(
                    shader,
                    len,
                    &mut len,
                    info_log.as_mut_ptr() as *mut gl::GLchar,
                );
                String::from_utf8_unchecked(info_log)
            }
        }

        #[inline]
        unsafe fn get_shader_parameter(
            &self,
            shader: Self::Shader,
            pname: gl::GLenum,
        ) -> gl::GLint {
            let mut param: gl::GLint = 0;
            unsafe { self.api.GetShaderiv(shader, pname, &mut param) };
            param
        }

        #[inline]
        unsafe fn get_string(&self, name: gl::GLenum) -> Option<String> {
            let ptr = unsafe { self.api.GetString(name) };
            if ptr.is_null() {
                return None;
            }
            let cstr = unsafe { CStr::from_ptr(ptr as *const c_char) };
            let string = cstr
                .to_str()
                .expect("string contains invalid utf8")
                .to_string();
            Some(string)
        }

        #[inline]
        unsafe fn get_uniform_location(
            &self,
            program: Self::Program,
            name: &CStr,
        ) -> Option<Self::UniformLocation> {
            let ret = unsafe { self.api.GetUniformLocation(program, name.as_ptr()) };
            gl::GLuint::try_from(ret).ok()
        }

        #[inline]
        unsafe fn link_program(&self, program: Self::Program) {
            unsafe { self.api.LinkProgram(program) };
        }

        #[inline]
        unsafe fn pixel_storei(&self, pname: gl::GLenum, param: gl::GLint) {
            unsafe { self.api.PixelStorei(pname, param) };
        }

        #[inline]
        unsafe fn read_buffer(&self, src: gl::GLenum) {
            unsafe { self.api.ReadBuffer(src) };
        }

        #[inline]
        unsafe fn read_pixels(
            &self,
            x: gl::GLint,
            y: gl::GLint,
            width: gl::GLsizei,
            height: gl::GLsizei,
            format: gl::GLenum,
            r#type: gl::GLenum,
            pixels: *mut c_void,
        ) {
            unsafe {
                self.api
                    .ReadPixels(x, y, width, height, format, r#type, pixels)
            };
        }

        #[inline]
        unsafe fn renderbuffer_storage(
            &self,
            target: gl::GLenum,
            internalformat: gl::GLenum,
            width: gl::GLsizei,
            height: gl::GLsizei,
        ) {
            unsafe {
                self.api
                    .RenderbufferStorage(target, internalformat, width, height)
            };
        }

        #[inline]
        unsafe fn scissor(
            &self,
            x: gl::GLint,
            y: gl::GLint,
            width: gl::GLsizei,
            height: gl::GLsizei,
        ) {
            unsafe { self.api.Scissor(x, y, width, height) };
        }

        #[inline]
        unsafe fn shader_source(&self, shader: Self::Shader, source: &str) {
            unsafe {
                self.api.ShaderSource(
                    shader,
                    1,
                    &(source.as_ptr() as *const gl::GLchar),
                    &(source.len() as gl::GLint),
                )
            };
        }

        #[inline]
        unsafe fn tex_image_2d(
            &self,
            target: gl::GLenum,
            level: gl::GLint,
            internalformat: gl::GLint,
            width: gl::GLsizei,
            height: gl::GLsizei,
            border: gl::GLint,
            format: gl::GLenum,
            r#type: gl::GLenum,
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
        unsafe fn tex_parameteri(&self, target: gl::GLenum, pname: gl::GLenum, param: gl::GLint) {
            unsafe { self.api.TexParameteri(target, pname, param) };
        }

        #[inline]
        unsafe fn tex_parameteriv(
            &self,
            target: gl::GLenum,
            pname: gl::GLenum,
            params: *const gl::GLint,
        ) {
            unsafe { self.api.TexParameteriv(target, pname, params) };
        }

        #[inline]
        unsafe fn tex_sub_image_2d(
            &self,
            target: gl::GLenum,
            level: gl::GLint,
            xoffset: gl::GLint,
            yoffset: gl::GLint,
            width: gl::GLsizei,
            height: gl::GLsizei,
            format: gl::GLenum,
            r#type: gl::GLenum,
            pixels: *const c_void,
        ) {
            unsafe {
                self.api.TexSubImage2D(
                    target, level, xoffset, yoffset, width, height, format, r#type, pixels,
                )
            };
        }

        #[inline]
        unsafe fn uniform_1f(&self, location: Self::UniformLocation, v0: gl::GLfloat) {
            unsafe { self.api.Uniform1f(location as gl::GLint, v0) };
        }

        #[inline]
        unsafe fn uniform_1i(&self, location: Self::UniformLocation, v0: gl::GLint) {
            unsafe { self.api.Uniform1i(location as gl::GLint, v0) };
        }

        #[inline]
        unsafe fn uniform_2f(
            &self,
            location: Self::UniformLocation,
            v0: gl::GLfloat,
            v1: gl::GLfloat,
        ) {
            unsafe { self.api.Uniform2f(location as gl::GLint, v0, v1) };
        }

        #[inline]
        unsafe fn uniform_4f(
            &self,
            location: Self::UniformLocation,
            v0: gl::GLfloat,
            v1: gl::GLfloat,
            v2: gl::GLfloat,
            v3: gl::GLfloat,
        ) {
            unsafe { self.api.Uniform4f(location as gl::GLint, v0, v1, v2, v3) };
        }

        #[inline]
        unsafe fn uniform_matrix_4fv(
            &self,
            location: Self::UniformLocation,
            count: gl::GLsizei,
            transpose: gl::GLboolean,
            value: *const gl::GLfloat,
        ) {
            unsafe {
                self.api
                    .UniformMatrix4fv(location as gl::GLint, count, transpose, value)
            };
        }

        #[inline]
        unsafe fn use_program(&self, program: Option<Self::Program>) {
            unsafe { self.api.UseProgram(program.unwrap_or(0)) };
        }

        #[inline]
        unsafe fn vertex_attrib_pointer(
            &self,
            index: gl::GLuint,
            size: gl::GLint,
            r#type: gl::GLenum,
            normalized: gl::GLboolean,
            stride: gl::GLsizei,
            pointer: *const c_void,
        ) {
            unsafe {
                self.api
                    .VertexAttribPointer(index, size, r#type, normalized, stride, pointer)
            };
        }

        #[inline]
        unsafe fn viewport(
            &self,
            x: gl::GLint,
            y: gl::GLint,
            width: gl::GLsizei,
            height: gl::GLsizei,
        ) {
            unsafe { self.api.Viewport(x, y, width, height) };
        }
    }
}

#[cfg(target_family = "wasm")]
mod webgl2 {
    use std::ffi::{CStr, c_void};

    use anyhow::Context as _;

    use super::Adapter;
    use crate::libgl as gl;

    fn new_typed_array(
        r#type: &str,
        buffer: js::Value,
        offset: usize,
        len: Option<usize>,
    ) -> Result<js::Value, js::Error> {
        let typed_array = js::GLOBAL.get(r#type);
        let args: &[js::Value] = match len {
            Some(len) => &[
                buffer,
                js::Value::from_f64(offset as f64),
                js::Value::from_f64(len as f64),
            ],
            None => &[buffer, js::Value::from_f64(offset as f64)],
        };
        typed_array.construct(args)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct JsHandle(u64);

    impl JsHandle {
        fn new(value: &js::Value) -> Self {
            Self(unsafe { std::mem::transmute_copy(value) })
        }
    }

    pub struct Api {
        activeTexture: js::Value,
        attachShader: js::Value,
        bindBuffer: js::Value,
        bindTexture: js::Value,
        bindVertexArray: js::Value,
        blendEquation: js::Value,
        blendFuncSeparate: js::Value,
        bufferData: js::Value,
        clear: js::Value,
        clearColor: js::Value,
        compileShader: js::Value,
        createBuffer: js::Value,
        createProgram: js::Value,
        createShader: js::Value,
        createTexture: js::Value,
        createVertexArray: js::Value,
        deleteShader: js::Value,
        deleteTexture: js::Value,
        detachShader: js::Value,
        drawBuffers: js::Value,
        drawElements: js::Value,
        enable: js::Value,
        enableVertexAttribArray: js::Value,
        getAttribLocation: js::Value,
        getProgramParameter: js::Value,
        getShaderInfoLog: js::Value,
        getShaderParameter: js::Value,
        getUniformLocation: js::Value,
        linkProgram: js::Value,
        pixelStorei: js::Value,
        shaderSource: js::Value,
        texImage2D: js::Value,
        texParameteri: js::Value,
        texSubImage2D: js::Value,
        uniform2f: js::Value,
        uniformMatrix4fv: js::Value,
        useProgram: js::Value,
        vertexAttribPointer: js::Value,
        viewport: js::Value,

        // TODO: experiment. replace with nohash if ok.
        handles: std::cell::RefCell<std::collections::HashMap<JsHandle, js::Value>>,

        _context: js::Value,
    }

    impl Api {
        pub fn from_canvas_selector(canvas_selector: &str) -> anyhow::Result<Self> {
            let canvas = js::GLOBAL
                .get("document")
                .get("querySelector")
                .call(&[js::Value::from_str(canvas_selector)])
                .context("could not find canvas")?;
            let context = canvas
                .get("getContext")
                .call(&[js::Value::from_str("webgl2")])
                .context("could not get context")?;

            Ok(Self {
                activeTexture: context.get("activeTexture"),
                attachShader: context.get("attachShader"),
                bindBuffer: context.get("bindBuffer"),
                bindTexture: context.get("bindTexture"),
                bindVertexArray: context.get("bindVertexArray"),
                blendEquation: context.get("blendEquation"),
                blendFuncSeparate: context.get("blendFuncSeparate"),
                bufferData: context.get("bufferData"),
                clear: context.get("clear"),
                clearColor: context.get("clearColor"),
                compileShader: context.get("compileShader"),
                createBuffer: context.get("createBuffer"),
                createProgram: context.get("createProgram"),
                createShader: context.get("createShader"),
                createTexture: context.get("createTexture"),
                createVertexArray: context.get("createVertexArray"),
                deleteShader: context.get("deleteShader"),
                deleteTexture: context.get("deleteTexture"),
                detachShader: context.get("detachShader"),
                drawBuffers: context.get("drawBuffers"),
                drawElements: context.get("drawElements"),
                enable: context.get("enable"),
                enableVertexAttribArray: context.get("enableVertexAttribArray"),
                getAttribLocation: context.get("getAttribLocation"),
                getProgramParameter: context.get("getProgramParameter"),
                getShaderInfoLog: context.get("getShaderInfoLog"),
                getShaderParameter: context.get("getShaderParameter"),
                getUniformLocation: context.get("getUniformLocation"),
                linkProgram: context.get("linkProgram"),
                pixelStorei: context.get("pixelStorei"),
                shaderSource: context.get("shaderSource"),
                texImage2D: context.get("texImage2D"),
                texParameteri: context.get("texParameteri"),
                texSubImage2D: context.get("texSubImage2D"),
                uniform2f: context.get("uniform2f"),
                uniformMatrix4fv: context.get("uniformMatrix4fv"),
                useProgram: context.get("useProgram"),
                vertexAttribPointer: context.get("vertexAttribPointer"),
                viewport: context.get("viewport"),

                handles: Default::default(),

                _context: context,
            })
        }

        fn insert_value(&self, value: js::Value) -> JsHandle {
            let handle = JsHandle::new(&value);
            self.handles.borrow_mut().insert(handle, value);
            handle
        }

        fn get_value(&self, handle: JsHandle) -> js::Value {
            self.handles.borrow().get(&handle).cloned().unwrap()
        }

        fn remove_value(&self, handle: JsHandle) -> js::Value {
            self.handles.borrow_mut().remove(&handle).unwrap()
        }
    }

    impl Adapter for Api {
        type Buffer = JsHandle;
        type Program = JsHandle;
        type Shader = JsHandle;
        type Texture = JsHandle;
        type UniformLocation = JsHandle;
        type VertexArray = JsHandle;

        #[inline]
        unsafe fn active_texture(&self, texture: gl::GLenum) {
            self.activeTexture
                .call(&[js::Value::from_f64(texture as f64)])
                .unwrap();
        }

        #[inline]
        unsafe fn attach_shader(&self, program: Self::Program, shader: Self::Shader) {
            let program = self.get_value(program);
            let shader = self.get_value(shader);
            self.attachShader.call(&[program, shader]).unwrap();
        }

        #[inline]
        unsafe fn bind_attrib_location(
            &self,
            _program: Self::Program,
            _index: gl::GLuint,
            _name: &CStr,
        ) {
            todo!()
        }

        #[inline]
        unsafe fn bind_buffer(&self, target: gl::GLenum, buffer: Option<Self::Buffer>) {
            let buffer = buffer.map_or_else(|| js::NULL, |handle| self.get_value(handle));
            self.bindBuffer
                .call(&[js::Value::from_f64(target as f64), buffer])
                .unwrap();
        }

        #[inline]
        unsafe fn bind_texture(&self, target: gl::GLenum, texture: Option<Self::Texture>) {
            let texture = texture.map_or_else(|| js::NULL, |handle| self.get_value(handle));
            self.bindTexture
                .call(&[js::Value::from_f64(target as f64), texture])
                .unwrap();
        }

        #[inline]
        unsafe fn bind_vertex_array(&self, array: Option<Self::VertexArray>) {
            let vertex_array = array.map_or_else(|| js::NULL, |handle| self.get_value(handle));
            self.bindVertexArray.call(&[vertex_array]).unwrap();
        }

        #[inline]
        unsafe fn blend_equation(&self, mode: gl::GLenum) {
            self.blendEquation
                .call(&[js::Value::from_f64(mode as f64)])
                .unwrap();
        }

        #[inline]
        unsafe fn blend_func_separate(
            &self,
            sfactor_rgb: gl::GLenum,
            dfactor_rgb: gl::GLenum,
            sfactor_alpha: gl::GLenum,
            dfactor_alpha: gl::GLenum,
        ) {
            self.blendFuncSeparate
                .call(&[
                    js::Value::from_f64(sfactor_rgb as f64),
                    js::Value::from_f64(dfactor_rgb as f64),
                    js::Value::from_f64(sfactor_alpha as f64),
                    js::Value::from_f64(dfactor_alpha as f64),
                ])
                .unwrap();
        }

        #[inline]
        unsafe fn buffer_data(
            &self,
            target: gl::GLenum,
            size: gl::GLsizeiptr,
            data: *const c_void,
            usage: gl::GLenum,
        ) {
            let memory_buffer = js::GLUE
                .get("instance")
                .get("exports")
                .get("memory")
                .get("buffer");
            let data = new_typed_array(
                "Uint8Array",
                memory_buffer,
                data as usize,
                Some(size as usize),
            )
            .expect("could not construct uint8 array");
            self.bufferData
                .call(&[
                    js::Value::from_f64(target as f64),
                    data,
                    js::Value::from_f64(usage as f64),
                ])
                .unwrap();
        }

        #[inline]
        unsafe fn clear(&self, mask: gl::GLbitfield) {
            self.clear
                .call(&[js::Value::from_f64(mask as f64)])
                .unwrap();
        }

        #[inline]
        unsafe fn clear_color(
            &self,
            red: gl::GLfloat,
            green: gl::GLfloat,
            blue: gl::GLfloat,
            alpha: gl::GLfloat,
        ) {
            self.clearColor
                .call(&[
                    js::Value::from_f64(red as f64),
                    js::Value::from_f64(green as f64),
                    js::Value::from_f64(blue as f64),
                    js::Value::from_f64(alpha as f64),
                ])
                .unwrap();
        }

        #[inline]
        unsafe fn compile_shader(&self, shader: Self::Shader) {
            let shader = self.get_value(shader);
            self.compileShader.call(&[shader]).unwrap();
        }

        #[inline]
        unsafe fn create_buffer(&self) -> Option<Self::Buffer> {
            let buffer = self.createBuffer.call(&[]).unwrap();
            Some(self.insert_value(buffer))
        }

        #[inline]
        unsafe fn create_program(&self) -> Option<Self::Program> {
            let program = self.createProgram.call(&[]).unwrap();
            Some(self.insert_value(program))
        }

        #[inline]
        unsafe fn create_shader(&self, r#type: gl::GLenum) -> Option<Self::Shader> {
            let shader = self
                .createShader
                .call(&[js::Value::from_f64(r#type as f64)])
                .unwrap();
            if shader == js::NULL {
                None
            } else {
                Some(self.insert_value(shader))
            }
        }

        #[inline]
        unsafe fn create_texture(&self) -> Option<Self::Texture> {
            let texture = self.createTexture.call(&[]).unwrap();
            Some(self.insert_value(texture))
        }

        #[inline]
        unsafe fn create_vertex_array(&self) -> Option<Self::VertexArray> {
            let vertex_array = self.createVertexArray.call(&[]).unwrap();
            Some(self.insert_value(vertex_array))
        }

        #[inline]
        unsafe fn delete_buffer(&self, _buffer: Self::Buffer) {
            todo!()
        }

        #[inline]
        unsafe fn delete_program(&self, _program: Self::Program) {
            todo!()
        }

        #[inline]
        unsafe fn delete_shader(&self, shader: Self::Shader) {
            let shader = self.remove_value(shader);
            self.deleteShader.call(&[shader]).unwrap();
        }

        #[inline]
        unsafe fn delete_texture(&self, texture: Self::Texture) {
            let texture = self.remove_value(texture);
            self.deleteTexture.call(&[texture]).unwrap();
        }

        #[inline]
        unsafe fn detach_shader(&self, program: Self::Program, shader: Self::Shader) {
            let program = self.get_value(program);
            let shader = self.get_value(shader);
            self.detachShader.call(&[program, shader]).unwrap();
        }

        #[inline]
        unsafe fn disable(&self, _cap: gl::GLenum) {
            todo!()
        }

        #[inline]
        unsafe fn draw_buffer(&self, buf: gl::GLenum) {
            let arrayof1 = js::GLOBAL
                .get("Array")
                .construct(&[js::Value::from_f64(1.0)])
                .unwrap();
            arrayof1.set("0", &js::Value::from_f64(buf as f64));
            self.drawBuffers.call(&[arrayof1]).unwrap();
        }

        #[inline]
        unsafe fn draw_elements(
            &self,
            mode: gl::GLenum,
            count: gl::GLsizei,
            r#type: gl::GLenum,
            indices: *const c_void,
        ) {
            self.drawElements
                .call(&[
                    js::Value::from_f64(mode as f64),
                    js::Value::from_f64(count as f64),
                    js::Value::from_f64(r#type as f64),
                    js::Value::from_f64(indices as usize as f64),
                ])
                .unwrap();
        }

        #[inline]
        unsafe fn enable(&self, cap: gl::GLenum) {
            self.enable
                .call(&[js::Value::from_f64(cap as f64)])
                .unwrap();
        }

        #[inline]
        unsafe fn enable_vertex_attrib_array(&self, index: gl::GLuint) {
            self.enableVertexAttribArray
                .call(&[js::Value::from_f64(index as f64)])
                .unwrap();
        }

        #[inline]
        unsafe fn get_attrib_location(
            &self,
            program: Self::Program,
            name: &CStr,
        ) -> Option<gl::GLint> {
            let program = self.get_value(program);
            let name = js::Value::from_str(name.to_str().expect("name contains invalid utf8"));
            let attrib_location = self
                .getAttribLocation
                .call(&[program, name])
                .unwrap()
                .as_f64() as gl::GLint;
            (attrib_location != -1).then_some(attrib_location)
        }

        #[inline]
        unsafe fn get_error(&self) -> Option<gl::GLenum> {
            todo!()
        }

        #[inline]
        unsafe fn get_program_info_log(&self, _program: Self::Program) -> String {
            todo!()
        }

        #[inline]
        unsafe fn get_program_parameter(
            &self,
            program: Self::Program,
            pname: gl::GLenum,
        ) -> gl::GLint {
            let program = self.get_value(program);
            let parameter = self
                .getProgramParameter
                .call(&[program, js::Value::from_f64(pname as f64)])
                .unwrap();
            match pname {
                gl::DELETE_STATUS | gl::LINK_STATUS | gl::VALIDATE_STATUS => {
                    parameter.as_bool() as gl::GLint
                }
                _ => parameter.as_f64() as gl::GLint,
            }
        }

        #[inline]
        unsafe fn get_shader_info_log(&self, shader: Self::Shader) -> String {
            let shader = self.get_value(shader);
            self.getShaderInfoLog.call(&[shader]).unwrap().as_string()
        }

        #[inline]
        unsafe fn get_shader_parameter(
            &self,
            shader: Self::Shader,
            pname: gl::GLenum,
        ) -> gl::GLint {
            let shader = self.get_value(shader);
            let parameter = self
                .getShaderParameter
                .call(&[shader, js::Value::from_f64(pname as f64)])
                .unwrap();
            match pname {
                gl::DELETE_STATUS | gl::COMPILE_STATUS => parameter.as_bool() as gl::GLint,
                _ => parameter.as_f64() as gl::GLint,
            }
        }

        #[inline]
        unsafe fn get_string(&self, _name: gl::GLenum) -> Option<String> {
            todo!()
        }

        #[inline]
        unsafe fn get_uniform_location(
            &self,
            program: Self::Program,
            name: &CStr,
        ) -> Option<Self::UniformLocation> {
            let program = self.get_value(program);
            let name = js::Value::from_str(name.to_str().expect("name contains invalid utf8"));
            let uniform_location = self.getUniformLocation.call(&[program, name]).unwrap();
            if uniform_location == js::NULL {
                None
            } else {
                Some(self.insert_value(uniform_location))
            }
        }

        #[inline]
        unsafe fn link_program(&self, program: Self::Program) {
            let program = self.get_value(program);
            self.linkProgram.call(&[program]).unwrap();
        }

        #[inline]
        unsafe fn pixel_storei(&self, pname: gl::GLenum, param: gl::GLint) {
            self.pixelStorei
                .call(&[
                    js::Value::from_f64(pname as f64),
                    js::Value::from_f64(param as f64),
                ])
                .unwrap();
        }

        #[inline]
        unsafe fn read_pixels(
            &self,
            _x: gl::GLint,
            _y: gl::GLint,
            _width: gl::GLsizei,
            _height: gl::GLsizei,
            _format: gl::GLenum,
            r#_type: gl::GLenum,
            _pixels: *mut c_void,
        ) {
            todo!()
        }

        #[inline]
        unsafe fn scissor(
            &self,
            _x: gl::GLint,
            _y: gl::GLint,
            _width: gl::GLsizei,
            _height: gl::GLsizei,
        ) {
            todo!()
        }

        #[inline]
        unsafe fn shader_source(&self, shader: Self::Shader, source: &str) {
            let shader = self.get_value(shader);
            self.shaderSource
                .call(&[shader, js::Value::from_str(source)])
                .unwrap();
        }

        #[inline]
        unsafe fn tex_image_2d(
            &self,
            target: gl::GLenum,
            level: gl::GLint,
            internalformat: gl::GLint,
            width: gl::GLsizei,
            height: gl::GLsizei,
            border: gl::GLint,
            format: gl::GLenum,
            r#type: gl::GLenum,
            pixels: *const c_void,
        ) {
            let memory_buffer = js::GLUE
                .get("instance")
                .get("exports")
                .get("memory")
                .get("buffer");
            let pixels = new_typed_array("Uint8Array", memory_buffer, pixels as usize, None)
                .expect("could not construct uint8 array");

            self.texImage2D
                .call(&[
                    js::Value::from_f64(target as f64),
                    js::Value::from_f64(level as f64),
                    js::Value::from_f64(internalformat as f64),
                    js::Value::from_f64(width as f64),
                    js::Value::from_f64(height as f64),
                    js::Value::from_f64(border as f64),
                    js::Value::from_f64(format as f64),
                    js::Value::from_f64(r#type as f64),
                    pixels,
                ])
                .unwrap();
        }

        #[inline]
        unsafe fn tex_parameteri(&self, target: gl::GLenum, pname: gl::GLenum, param: gl::GLint) {
            self.texParameteri
                .call(&[
                    js::Value::from_f64(target as f64),
                    js::Value::from_f64(pname as f64),
                    js::Value::from_f64(param as f64),
                ])
                .unwrap();
        }

        #[inline]
        unsafe fn tex_parameteriv(
            &self,
            _target: gl::GLenum,
            _pname: gl::GLenum,
            _params: *const gl::GLint,
        ) {
            todo!()
        }

        #[inline]
        unsafe fn tex_sub_image_2d(
            &self,
            target: gl::GLenum,
            level: gl::GLint,
            xoffset: gl::GLint,
            yoffset: gl::GLint,
            width: gl::GLsizei,
            height: gl::GLsizei,
            format: gl::GLenum,
            r#type: gl::GLenum,
            pixels: *const c_void,
        ) {
            let memory_buffer = js::GLUE
                .get("instance")
                .get("exports")
                .get("memory")
                .get("buffer");
            let pixels = new_typed_array("Uint8Array", memory_buffer, pixels as usize, None)
                .expect("could not construct uint8 array");

            self.texSubImage2D
                .call(&[
                    js::Value::from_f64(target as f64),
                    js::Value::from_f64(level as f64),
                    js::Value::from_f64(xoffset as f64),
                    js::Value::from_f64(yoffset as f64),
                    js::Value::from_f64(width as f64),
                    js::Value::from_f64(height as f64),
                    js::Value::from_f64(format as f64),
                    js::Value::from_f64(r#type as f64),
                    pixels,
                ])
                .unwrap();
        }

        #[inline]
        unsafe fn uniform_1f(&self, _location: Self::UniformLocation, _v0: gl::GLfloat) {
            todo!()
        }

        #[inline]
        unsafe fn uniform_1i(&self, _location: Self::UniformLocation, _v0: gl::GLint) {
            todo!()
        }

        #[inline]
        unsafe fn uniform_2f(
            &self,
            location: Self::UniformLocation,
            v0: gl::GLfloat,
            v1: gl::GLfloat,
        ) {
            self.uniform2f
                .call(&[
                    self.get_value(location),
                    js::Value::from_f64(v0 as f64),
                    js::Value::from_f64(v1 as f64),
                ])
                .unwrap();
        }

        #[inline]
        unsafe fn uniform_matrix_4fv(
            &self,
            location: Self::UniformLocation,
            count: gl::GLsizei,
            transpose: gl::GLboolean,
            value: *const gl::GLfloat,
        ) {
            // TODO: iterate and upload multiple matrix4
            assert_eq!(count, 1);
            let memory_buffer = js::GLUE
                .get("instance")
                .get("exports")
                .get("memory")
                .get("buffer");
            let value = new_typed_array("Float32Array", memory_buffer, value as usize, Some(4 * 4))
                .expect("could not construct float32 array");
            self.uniformMatrix4fv
                .call(&[
                    self.get_value(location),
                    js::Value::from_bool(transpose != 0),
                    value,
                ])
                .unwrap();
        }

        #[inline]
        unsafe fn use_program(&self, program: Option<Self::Program>) {
            let program = program.map_or_else(|| js::NULL, |handle| self.get_value(handle));
            self.useProgram.call(&[program]).unwrap();
        }

        #[inline]
        unsafe fn vertex_attrib_pointer(
            &self,
            index: gl::GLuint,
            size: gl::GLint,
            r#type: gl::GLenum,
            normalized: gl::GLboolean,
            stride: gl::GLsizei,
            pointer: *const c_void,
        ) {
            // NOTE: pointer is an offset. not an actual poitner-pointer.
            self.vertexAttribPointer
                .call(&[
                    js::Value::from_f64(index as f64),
                    js::Value::from_f64(size as f64),
                    js::Value::from_f64(r#type as f64),
                    js::Value::from_bool(normalized != 0),
                    js::Value::from_f64(stride as f64),
                    js::Value::from_f64(pointer as usize as f64),
                ])
                .unwrap();
        }

        #[inline]
        unsafe fn viewport(
            &self,
            x: gl::GLint,
            y: gl::GLint,
            width: gl::GLsizei,
            height: gl::GLsizei,
        ) {
            self.viewport
                .call(&[
                    js::Value::from_f64(x as f64),
                    js::Value::from_f64(y as f64),
                    js::Value::from_f64(width as f64),
                    js::Value::from_f64(height as f64),
                ])
                .unwrap();
        }
    }
}

#[cfg(not(target_family = "wasm"))]
pub use gl46::*;

#[cfg(target_family = "wasm")]
pub use webgl2::*;

pub type Buffer = <Api as Adapter>::Buffer;
pub type Framebuffer = <Api as Adapter>::Framebuffer;
pub type Program = <Api as Adapter>::Program;
pub type Renderbuffer = <Api as Adapter>::Renderbuffer;
pub type Shader = <Api as Adapter>::Shader;
pub type Texture = <Api as Adapter>::Texture;
pub type UniformLocation = <Api as Adapter>::UniformLocation;
pub type VertexArray = <Api as Adapter>::VertexArray;
