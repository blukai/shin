use std::ffi::{CStr, c_void};

use crate::libgl as gl;

// NOTE: why not just use glow?
// i very don't like that its Api trait does not exactly mirror gl spec.
// i don't like that it abstracts away certain thigs, i don't want that.
//
// TODO: make sure that all methods match libgl's methods 1:1 with the exception of things that can
// be rustified (like strings and stuff).
pub trait Adapter {
    type Buffer;
    type Program;
    type Shader;
    type Texture;

    unsafe fn active_texture(&self, texture: gl::GLenum);
    unsafe fn attach_shader(&self, program: Self::Program, shader: Self::Shader);
    unsafe fn bind_attrib_location(&self, program: Self::Program, index: gl::GLuint, name: &CStr);
    unsafe fn bind_buffer(&self, target: gl::GLenum, buffer: Option<Self::Buffer>);
    unsafe fn bind_texture(&self, target: gl::GLenum, texture: Option<Self::Texture>);
    unsafe fn blend_equation(&self, mode: gl::GLenum);
    unsafe fn blend_func_separate(
        &self,
        src_rgb: gl::GLenum,
        dst_rgb: gl::GLenum,
        src_alpha: gl::GLenum,
        dst_alpha: gl::GLenum,
    );
    unsafe fn buffer_data(
        &self,
        target: gl::GLenum,
        size: gl::GLsizeiptr,
        data: *const c_void,
        usage: gl::GLenum,
    );
    unsafe fn clear(&self, mask: gl::GLbitfield);
    unsafe fn clear_color(
        &self,
        red: gl::GLfloat,
        green: gl::GLfloat,
        blue: gl::GLfloat,
        alpha: gl::GLfloat,
    );
    unsafe fn compile_shader(&self, shader: Self::Shader);
    unsafe fn create_buffer(&self) -> anyhow::Result<Self::Buffer>;
    unsafe fn create_program(&self) -> anyhow::Result<Self::Program>;
    unsafe fn create_shader(&self, r#type: gl::GLenum) -> anyhow::Result<Self::Shader>;
    unsafe fn create_texture(&self) -> anyhow::Result<Self::Texture>;
    unsafe fn delete_buffer(&self, buffer: Self::Buffer);
    unsafe fn delete_program(&self, program: Self::Program);
    unsafe fn delete_shader(&self, shader: Self::Shader);
    unsafe fn delete_texture(&self, texture: Self::Texture);
    unsafe fn detach_shader(&self, program: Self::Program, shader: Self::Shader);
    unsafe fn disable(&self, cap: gl::GLenum);
    unsafe fn draw_elements(
        &self,
        mode: gl::GLenum,
        count: gl::GLsizei,
        r#type: gl::GLenum,
        indices: *const c_void,
    );
    unsafe fn enable(&self, cap: gl::GLenum);
    unsafe fn enable_vertex_attrib_array(&self, index: gl::GLuint);
    unsafe fn get_attrib_location(&self, program: Self::Program, name: &CStr) -> Option<gl::GLint>;
    unsafe fn get_error(&self) -> Option<gl::GLenum>;
    unsafe fn get_program_info_log(&self, program: Self::Program) -> String;
    // TODO: same issue as with get_shader_parameter.
    unsafe fn get_program_parameter(&self, program: Self::Program, pname: gl::GLenum) -> gl::GLint;
    // TODO: don't force allocations, maybe you want to re-use existing allocations.
    unsafe fn get_shader_info_log(&self, shader: Self::Shader) -> String;
    // TODO: why the fuck would you want to rename getshaderiv to get_shader_parameter? be
    // consistent!
    unsafe fn get_shader_parameter(&self, shader: Self::Shader, pname: gl::GLenum) -> gl::GLint;
    unsafe fn get_string(&self, name: gl::GLenum) -> anyhow::Result<String>;
    unsafe fn get_uniform_location(&self, program: Self::Program, name: &CStr)
    -> Option<gl::GLint>;
    unsafe fn link_program(&self, program: Self::Program);
    unsafe fn pixel_storei(&self, pname: gl::GLenum, param: gl::GLint);
    unsafe fn read_pixels(
        &self,
        x: gl::GLint,
        y: gl::GLint,
        width: gl::GLsizei,
        height: gl::GLsizei,
        format: gl::GLenum,
        r#type: gl::GLenum,
        pixels: *mut c_void,
    );
    unsafe fn scissor(&self, x: gl::GLint, y: gl::GLint, width: gl::GLsizei, height: gl::GLsizei);
    unsafe fn shader_source(&self, shader: Self::Shader, source: &str);
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
    );
    unsafe fn tex_parameteri(&self, target: gl::GLenum, pname: gl::GLenum, param: gl::GLint);
    unsafe fn tex_parameteriv(
        &self,
        target: gl::GLenum,
        pname: gl::GLenum,
        params: *const gl::GLint,
    );
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
    );
    unsafe fn uniform_1f(&self, location: gl::GLint, v0: gl::GLfloat);
    unsafe fn uniform_1i(&self, location: gl::GLint, v0: gl::GLint);
    unsafe fn uniform_2f(&self, location: gl::GLint, v0: gl::GLfloat, v1: gl::GLfloat);
    unsafe fn use_program(&self, program: Option<Self::Program>);
    unsafe fn vertex_attrib_pointer(
        &self,
        index: gl::GLuint,
        size: gl::GLint,
        r#type: gl::GLenum,
        normalized: gl::GLboolean,
        stride: gl::GLsizei,
        pointer: *const c_void,
    );
    unsafe fn viewport(&self, x: gl::GLint, y: gl::GLint, width: gl::GLsizei, height: gl::GLsizei);
}

#[cfg(not(target_family = "wasm"))]
mod gl46 {
    use std::ffi::{CStr, c_char, c_void};
    use std::num::NonZero;

    use anyhow::{Context as _, anyhow};

    use super::Adapter;
    use crate::libgl as gl;

    pub use crate::libgl::Api;

    // TODO: do not implement this on libgl::Api, but in a separate api please. it's easy to mix
    // adapter stuff into non-adapter env.
    impl Adapter for Api {
        type Buffer = NonZero<gl::GLuint>;
        type Program = NonZero<gl::GLuint>;
        type Shader = NonZero<gl::GLuint>;
        type Texture = NonZero<gl::GLuint>;

        #[inline]
        unsafe fn active_texture(&self, texture: gl::GLenum) {
            unsafe { self.ActiveTexture(texture) };
        }

        #[inline]
        unsafe fn attach_shader(&self, program: Self::Program, shader: Self::Shader) {
            unsafe { self.AttachShader(program.get(), shader.get()) };
        }

        #[inline]
        unsafe fn bind_attrib_location(
            &self,
            program: Self::Program,
            index: gl::GLuint,
            name: &CStr,
        ) {
            unsafe { self.BindAttribLocation(program.get(), index, name.as_ptr()) };
        }

        #[inline]
        unsafe fn bind_buffer(&self, target: gl::GLenum, buffer: Option<Self::Buffer>) {
            unsafe { self.BindBuffer(target, buffer.map_or_else(|| 0, |v| v.get())) };
        }

        #[inline]
        unsafe fn bind_texture(&self, target: gl::GLenum, texture: Option<Self::Texture>) {
            unsafe { self.BindTexture(target, texture.map_or_else(|| 0, |v| v.get())) };
        }

        #[inline]
        unsafe fn blend_equation(&self, mode: gl::GLenum) {
            unsafe { self.BlendEquation(mode) };
        }

        #[inline]
        unsafe fn blend_func_separate(
            &self,
            src_rgb: gl::GLenum,
            dst_rgb: gl::GLenum,
            src_alpha: gl::GLenum,
            dst_alpha: gl::GLenum,
        ) {
            unsafe { self.BlendFuncSeparate(src_rgb, dst_rgb, src_alpha, dst_alpha) };
        }

        #[inline]
        unsafe fn buffer_data(
            &self,
            target: gl::GLenum,
            size: gl::GLsizeiptr,
            data: *const c_void,
            usage: gl::GLenum,
        ) {
            unsafe { self.BufferData(target, size, data, usage) };
        }

        #[inline]
        unsafe fn clear(&self, mask: gl::GLbitfield) {
            unsafe { self.Clear(mask) };
        }

        #[inline]
        unsafe fn clear_color(
            &self,
            red: gl::GLfloat,
            green: gl::GLfloat,
            blue: gl::GLfloat,
            alpha: gl::GLfloat,
        ) {
            unsafe { self.ClearColor(red, green, blue, alpha) };
        }

        #[inline]
        unsafe fn compile_shader(&self, shader: Self::Shader) {
            unsafe { self.CompileShader(shader.get()) };
        }

        #[inline]
        unsafe fn create_buffer(&self) -> anyhow::Result<Self::Buffer> {
            let mut buffer: gl::GLuint = 0;
            unsafe { self.GenBuffers(1, &mut buffer) };
            NonZero::new(buffer).context("could not create buffer")
        }

        #[inline]
        unsafe fn create_program(&self) -> anyhow::Result<Self::Program> {
            let program = unsafe { self.CreateProgram() };
            NonZero::new(program).context("could not create program")
        }

        #[inline]
        unsafe fn create_shader(&self, r#type: gl::GLenum) -> anyhow::Result<Self::Shader> {
            let program = unsafe { self.CreateShader(r#type) };
            NonZero::new(program).context("could not create shader")
        }

        #[inline]
        unsafe fn create_texture(&self) -> anyhow::Result<Self::Texture> {
            let mut texture: gl::GLuint = 0;
            unsafe { self.GenTextures(1, &mut texture) };
            NonZero::new(texture).context("could not create texture")
        }

        #[inline]
        unsafe fn delete_buffer(&self, buffer: Self::Buffer) {
            unsafe { self.DeleteBuffers(1, &buffer.get()) };
        }

        #[inline]
        unsafe fn delete_program(&self, program: Self::Program) {
            unsafe { self.DeleteProgram(program.get()) };
        }

        #[inline]
        unsafe fn delete_shader(&self, shader: Self::Shader) {
            unsafe { self.DeleteShader(shader.get()) };
        }

        #[inline]
        unsafe fn delete_texture(&self, texture: Self::Texture) {
            unsafe { self.DeleteTextures(1, &texture.get()) };
        }

        #[inline]
        unsafe fn detach_shader(&self, program: Self::Program, shader: Self::Shader) {
            unsafe { self.DetachShader(program.get(), shader.get()) };
        }

        #[inline]
        unsafe fn disable(&self, cap: gl::GLenum) {
            unsafe { self.Disable(cap) };
        }

        #[inline]
        unsafe fn draw_elements(
            &self,
            mode: gl::GLenum,
            count: gl::GLsizei,
            r#type: gl::GLenum,
            indices: *const c_void,
        ) {
            unsafe { self.DrawElements(mode, count, r#type, indices) }
        }

        #[inline]
        unsafe fn enable(&self, cap: gl::GLenum) {
            unsafe { self.Enable(cap) };
        }

        #[inline]
        unsafe fn enable_vertex_attrib_array(&self, index: gl::GLuint) {
            unsafe { self.EnableVertexAttribArray(index) };
        }

        #[inline]
        unsafe fn get_attrib_location(
            &self,
            program: Self::Program,
            name: &CStr,
        ) -> Option<gl::GLint> {
            let ret = unsafe { self.GetAttribLocation(program.get(), name.as_ptr()) };
            (ret != -1).then_some(ret)
        }

        #[inline]
        unsafe fn get_error(&self) -> Option<gl::GLenum> {
            let ret = unsafe { self.GetError() };
            (ret != gl::NO_ERROR).then_some(ret)
        }

        #[inline]
        unsafe fn get_program_info_log(&self, program: Self::Program) -> String {
            let mut len = unsafe { self.get_shader_parameter(program, gl::INFO_LOG_LENGTH) };
            let mut info_log = vec![0; len as usize];
            unsafe {
                self.GetProgramInfoLog(
                    program.get(),
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
            unsafe { self.GetProgramiv(program.get(), pname, &mut param) };
            param
        }

        #[inline]
        unsafe fn get_shader_info_log(&self, shader: Self::Shader) -> String {
            let mut len = unsafe { self.get_shader_parameter(shader, gl::INFO_LOG_LENGTH) };
            let mut info_log = vec![0; len as usize];
            unsafe {
                self.GetShaderInfoLog(
                    shader.get(),
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
            unsafe { self.GetShaderiv(shader.get(), pname, &mut param) };
            param
        }

        #[inline]
        unsafe fn get_string(&self, name: gl::GLenum) -> anyhow::Result<String> {
            let ptr = unsafe { self.GetString(name) };
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
        unsafe fn get_uniform_location(
            &self,
            program: Self::Program,
            name: &CStr,
        ) -> Option<gl::GLint> {
            let ret = unsafe { self.GetUniformLocation(program.get(), name.as_ptr()) };
            (ret != -1).then_some(ret)
        }

        #[inline]
        unsafe fn link_program(&self, program: Self::Program) {
            unsafe { self.LinkProgram(program.get()) };
        }

        #[inline]
        unsafe fn pixel_storei(&self, pname: gl::GLenum, param: gl::GLint) {
            unsafe { self.PixelStorei(pname, param) };
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
            unsafe { self.ReadPixels(x, y, width, height, format, r#type, pixels) };
        }

        #[inline]
        unsafe fn scissor(
            &self,
            x: gl::GLint,
            y: gl::GLint,
            width: gl::GLsizei,
            height: gl::GLsizei,
        ) {
            unsafe { self.Scissor(x, y, width, height) };
        }

        #[inline]
        unsafe fn shader_source(&self, shader: Self::Shader, source: &str) {
            unsafe {
                self.ShaderSource(
                    shader.get(),
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
                self.TexImage2D(
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
            unsafe { self.TexParameteri(target, pname, param) };
        }

        #[inline]
        unsafe fn tex_parameteriv(
            &self,
            target: gl::GLenum,
            pname: gl::GLenum,
            params: *const gl::GLint,
        ) {
            unsafe { self.TexParameteriv(target, pname, params) };
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
                self.TexSubImage2D(
                    target, level, xoffset, yoffset, width, height, format, r#type, pixels,
                )
            };
        }

        #[inline]
        unsafe fn uniform_1f(&self, location: gl::GLint, v0: gl::GLfloat) {
            unsafe { self.Uniform1f(location, v0) };
        }

        #[inline]
        unsafe fn uniform_1i(&self, location: gl::GLint, v0: gl::GLint) {
            unsafe { self.Uniform1i(location, v0) };
        }

        #[inline]
        unsafe fn uniform_2f(&self, location: gl::GLint, v0: gl::GLfloat, v1: gl::GLfloat) {
            unsafe { self.Uniform2f(location, v0, v1) };
        }

        #[inline]
        unsafe fn use_program(&self, program: Option<Self::Program>) {
            unsafe { self.UseProgram(program.map_or_else(|| 0, |v| v.get())) };
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
            unsafe { self.VertexAttribPointer(index, size, r#type, normalized, stride, pointer) };
        }

        #[inline]
        unsafe fn viewport(
            &self,
            x: gl::GLint,
            y: gl::GLint,
            width: gl::GLsizei,
            height: gl::GLsizei,
        ) {
            unsafe { self.Viewport(x, y, width, height) };
        }
    }
}

#[cfg(target_family = "wasm")]
mod webgl2 {
    use std::ffi::{CStr, c_void};

    use anyhow::Context as _;

    use super::Adapter;
    use crate::libgl as gl;

    pub struct Api {
        clear: js::Value,
        clear_color: js::Value,

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
                clear: context.get("clear"),
                clear_color: context.get("clearColor"),

                _context: context,
            })
        }
    }

    impl Adapter for Api {
        type Buffer = u32;
        type Program = u32;
        type Shader = u32;
        type Texture = u32;

        #[inline]
        unsafe fn active_texture(&self, _texture: gl::GLenum) {
            todo!()
        }

        #[inline]
        unsafe fn attach_shader(&self, _program: Self::Program, _shader: Self::Shader) {
            todo!()
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
        unsafe fn bind_buffer(&self, _target: gl::GLenum, _buffer: Option<Self::Buffer>) {
            todo!()
        }

        #[inline]
        unsafe fn bind_texture(&self, _target: gl::GLenum, _texture: Option<Self::Texture>) {
            todo!()
        }

        #[inline]
        unsafe fn blend_equation(&self, _mode: gl::GLenum) {
            todo!()
        }

        #[inline]
        unsafe fn blend_func_separate(
            &self,
            _src_rgb: gl::GLenum,
            _dst_rgb: gl::GLenum,
            _src_alpha: gl::GLenum,
            _dst_alpha: gl::GLenum,
        ) {
            todo!()
        }

        #[inline]
        unsafe fn buffer_data(
            &self,
            _target: gl::GLenum,
            _size: gl::GLsizeiptr,
            _data: *const c_void,
            _usage: gl::GLenum,
        ) {
            todo!()
        }

        #[inline]
        unsafe fn clear(&self, mask: gl::GLbitfield) {
            _ = self.clear.call(&[js::Value::from_f64(mask as f64)]);
        }

        #[inline]
        unsafe fn clear_color(
            &self,
            red: gl::GLfloat,
            green: gl::GLfloat,
            blue: gl::GLfloat,
            alpha: gl::GLfloat,
        ) {
            _ = self.clear_color.call(&[
                js::Value::from_f64(red as f64),
                js::Value::from_f64(green as f64),
                js::Value::from_f64(blue as f64),
                js::Value::from_f64(alpha as f64),
            ]);
        }

        #[inline]
        unsafe fn compile_shader(&self, _shader: Self::Shader) {
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
        unsafe fn create_shader(&self, r#_type: gl::GLenum) -> anyhow::Result<Self::Shader> {
            todo!()
        }

        #[inline]
        unsafe fn create_texture(&self) -> anyhow::Result<Self::Texture> {
            todo!()
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
        unsafe fn delete_shader(&self, _shader: Self::Shader) {
            todo!()
        }

        #[inline]
        unsafe fn delete_texture(&self, _texture: Self::Texture) {
            todo!()
        }

        #[inline]
        unsafe fn detach_shader(&self, _program: Self::Program, _shader: Self::Shader) {
            todo!()
        }

        #[inline]
        unsafe fn disable(&self, _cap: gl::GLenum) {
            todo!()
        }

        #[inline]
        unsafe fn draw_elements(
            &self,
            _mode: gl::GLenum,
            _count: gl::GLsizei,
            r#_type: gl::GLenum,
            _indices: *const c_void,
        ) {
            todo!()
        }

        #[inline]
        unsafe fn enable(&self, _cap: gl::GLenum) {
            todo!()
        }

        #[inline]
        unsafe fn enable_vertex_attrib_array(&self, _index: gl::GLuint) {
            todo!()
        }

        #[inline]
        unsafe fn get_attrib_location(
            &self,
            _program: Self::Program,
            _name: &CStr,
        ) -> Option<gl::GLint> {
            todo!()
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
            _program: Self::Program,
            _pname: gl::GLenum,
        ) -> gl::GLint {
            todo!()
        }

        #[inline]
        unsafe fn get_shader_info_log(&self, _shader: Self::Shader) -> String {
            todo!()
        }

        #[inline]
        unsafe fn get_shader_parameter(
            &self,
            _shader: Self::Shader,
            _pname: gl::GLenum,
        ) -> gl::GLint {
            todo!()
        }

        #[inline]
        unsafe fn get_string(&self, _name: gl::GLenum) -> anyhow::Result<String> {
            todo!()
        }

        #[inline]
        unsafe fn get_uniform_location(
            &self,
            _program: Self::Program,
            _name: &CStr,
        ) -> Option<gl::GLint> {
            todo!()
        }

        #[inline]
        unsafe fn link_program(&self, _program: Self::Program) {
            todo!()
        }

        #[inline]
        unsafe fn pixel_storei(&self, _pname: gl::GLenum, _param: gl::GLint) {
            todo!()
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
        unsafe fn shader_source(&self, _shader: Self::Shader, _source: &str) {
            todo!()
        }

        #[inline]
        unsafe fn tex_image_2d(
            &self,
            _target: gl::GLenum,
            _level: gl::GLint,
            _internalformat: gl::GLint,
            _width: gl::GLsizei,
            _height: gl::GLsizei,
            _border: gl::GLint,
            _format: gl::GLenum,
            r#_type: gl::GLenum,
            _pixels: *const c_void,
        ) {
            todo!()
        }

        #[inline]
        unsafe fn tex_parameteri(
            &self,
            _target: gl::GLenum,
            _pname: gl::GLenum,
            _param: gl::GLint,
        ) {
            todo!()
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
            _target: gl::GLenum,
            _level: gl::GLint,
            _xoffset: gl::GLint,
            _yoffset: gl::GLint,
            _width: gl::GLsizei,
            _height: gl::GLsizei,
            _format: gl::GLenum,
            r#_type: gl::GLenum,
            _pixels: *const c_void,
        ) {
            todo!()
        }

        #[inline]
        unsafe fn uniform_1f(&self, _location: gl::GLint, _v0: gl::GLfloat) {
            todo!()
        }

        #[inline]
        unsafe fn uniform_1i(&self, _location: gl::GLint, _v0: gl::GLint) {
            todo!()
        }

        #[inline]
        unsafe fn uniform_2f(&self, _location: gl::GLint, _v0: gl::GLfloat, _v1: gl::GLfloat) {
            todo!()
        }

        #[inline]
        unsafe fn use_program(&self, _program: Option<Self::Program>) {
            todo!()
        }

        #[inline]
        unsafe fn vertex_attrib_pointer(
            &self,
            _index: gl::GLuint,
            _size: gl::GLint,
            r#_type: gl::GLenum,
            _normalized: gl::GLboolean,
            _stride: gl::GLsizei,
            _pointer: *const c_void,
        ) {
            todo!()
        }

        #[inline]
        unsafe fn viewport(
            &self,
            _x: gl::GLint,
            _y: gl::GLint,
            _width: gl::GLsizei,
            _height: gl::GLsizei,
        ) {
            todo!()
        }
    }
}

#[cfg(not(target_family = "wasm"))]
pub use gl46::*;

#[cfg(target_family = "wasm")]
pub use webgl2::*;

pub type Buffer = <Api as Adapter>::Buffer;
pub type Program = <Api as Adapter>::Program;
pub type Shader = <Api as Adapter>::Shader;
pub type Texture = <Api as Adapter>::Texture;
