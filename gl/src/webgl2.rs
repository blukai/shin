use std::ffi::{CStr, c_void};

use anyhow::Context as _;

use super::Adapter;
use super::types::*;

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
    unsafe fn active_texture(&self, _texture: GLenum) {
        todo!()
    }

    #[inline]
    unsafe fn attach_shader(&self, _program: Self::Program, _shader: Self::Shader) {
        todo!()
    }

    #[inline]
    unsafe fn bind_attrib_location(&self, _program: Self::Program, _index: GLuint, _name: &CStr) {
        todo!()
    }

    #[inline]
    unsafe fn bind_buffer(&self, _target: GLenum, _buffer: Option<Self::Buffer>) {
        todo!()
    }

    #[inline]
    unsafe fn bind_texture(&self, _target: GLenum, _texture: Option<Self::Texture>) {
        todo!()
    }

    #[inline]
    unsafe fn blend_equation(&self, _mode: GLenum) {
        todo!()
    }

    #[inline]
    unsafe fn blend_func_separate(
        &self,
        _src_rgb: GLenum,
        _dst_rgb: GLenum,
        _src_alpha: GLenum,
        _dst_alpha: GLenum,
    ) {
        todo!()
    }

    #[inline]
    unsafe fn buffer_data(
        &self,
        _target: GLenum,
        _size: GLsizeiptr,
        _data: *const c_void,
        _usage: GLenum,
    ) {
        todo!()
    }

    #[inline]
    unsafe fn clear(&self, mask: GLbitfield) {
        _ = self.clear.call(&[js::Value::from_f64(mask as f64)]);
    }

    #[inline]
    unsafe fn clear_color(&self, red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) {
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
    unsafe fn create_shader(&self, r#_type: GLenum) -> anyhow::Result<Self::Shader> {
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
    unsafe fn disable(&self, _cap: GLenum) {
        todo!()
    }

    #[inline]
    unsafe fn draw_elements(
        &self,
        _mode: GLenum,
        _count: GLsizei,
        r#_type: GLenum,
        _indices: *const c_void,
    ) {
        todo!()
    }

    #[inline]
    unsafe fn enable(&self, _cap: GLenum) {
        todo!()
    }

    #[inline]
    unsafe fn enable_vertex_attrib_array(&self, _index: GLuint) {
        todo!()
    }

    #[inline]
    unsafe fn get_attrib_location(&self, _program: Self::Program, _name: &CStr) -> Option<GLint> {
        todo!()
    }

    #[inline]
    unsafe fn get_error(&self) -> Option<GLenum> {
        todo!()
    }

    #[inline]
    unsafe fn get_program_info_log(&self, _program: Self::Program) -> String {
        todo!()
    }

    #[inline]
    unsafe fn get_program_parameter(&self, _program: Self::Program, _pname: GLenum) -> GLint {
        todo!()
    }

    #[inline]
    unsafe fn get_shader_info_log(&self, _shader: Self::Shader) -> String {
        todo!()
    }

    #[inline]
    unsafe fn get_shader_parameter(&self, _shader: Self::Shader, _pname: GLenum) -> GLint {
        todo!()
    }

    #[inline]
    unsafe fn get_string(&self, _name: GLenum) -> anyhow::Result<String> {
        todo!()
    }

    #[inline]
    unsafe fn get_uniform_location(&self, _program: Self::Program, _name: &CStr) -> Option<GLint> {
        todo!()
    }

    #[inline]
    unsafe fn link_program(&self, _program: Self::Program) {
        todo!()
    }

    #[inline]
    unsafe fn pixel_storei(&self, _pname: GLenum, _param: GLint) {
        todo!()
    }

    #[inline]
    unsafe fn read_pixels(
        &self,
        _x: GLint,
        _y: GLint,
        _width: GLsizei,
        _height: GLsizei,
        _format: GLenum,
        r#_type: GLenum,
        _pixels: *mut c_void,
    ) {
        todo!()
    }

    #[inline]
    unsafe fn scissor(&self, _x: GLint, _y: GLint, _width: GLsizei, _height: GLsizei) {
        todo!()
    }

    #[inline]
    unsafe fn shader_source(&self, _shader: Self::Shader, _source: &str) {
        todo!()
    }

    #[inline]
    unsafe fn tex_image_2d(
        &self,
        _target: GLenum,
        _level: GLint,
        _internalformat: GLint,
        _width: GLsizei,
        _height: GLsizei,
        _border: GLint,
        _format: GLenum,
        r#_type: GLenum,
        _pixels: *const c_void,
    ) {
        todo!()
    }

    #[inline]
    unsafe fn tex_parameteri(&self, _target: GLenum, _pname: GLenum, _param: GLint) {
        todo!()
    }

    #[inline]
    unsafe fn tex_parameteriv(&self, _target: GLenum, _pname: GLenum, _params: *const GLint) {
        todo!()
    }

    #[inline]
    unsafe fn tex_sub_image_2d(
        &self,
        _target: GLenum,
        _level: GLint,
        _xoffset: GLint,
        _yoffset: GLint,
        _width: GLsizei,
        _height: GLsizei,
        _format: GLenum,
        r#_type: GLenum,
        _pixels: *const c_void,
    ) {
        todo!()
    }

    #[inline]
    unsafe fn uniform_1f(&self, _location: GLint, _v0: GLfloat) {
        todo!()
    }

    #[inline]
    unsafe fn uniform_1i(&self, _location: GLint, _v0: GLint) {
        todo!()
    }

    #[inline]
    unsafe fn uniform_2f(&self, _location: GLint, _v0: GLfloat, _v1: GLfloat) {
        todo!()
    }

    #[inline]
    unsafe fn use_program(&self, _program: Option<Self::Program>) {
        todo!()
    }

    #[inline]
    unsafe fn vertex_attrib_pointer(
        &self,
        _index: GLuint,
        _size: GLint,
        r#_type: GLenum,
        _normalized: GLboolean,
        _stride: GLsizei,
        _pointer: *const c_void,
    ) {
        todo!()
    }

    #[inline]
    unsafe fn viewport(&self, _x: GLint, _y: GLint, _width: GLsizei, _height: GLsizei) {
        todo!()
    }
}
