#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::ffi::{c_char, c_uint, c_void};
use std::ptr::null_mut;

use dynlib::DynLib;

pub type khronos_int32_t = i32;
pub type khronos_utime_nanoseconds_t = u64;

// https://registry.khronos.org/EGL/api/EGL/eglplatform.h

pub type EGLNativeDisplayType = *mut c_void;
pub type EGLNativePixmapType = *mut c_void;
pub type EGLNativeWindowType = *mut c_void;

pub type EGLint = khronos_int32_t;

// https://registry.khronos.org/EGL/api/EGL/egl.h

// 1.0

pub type EGLBoolean = c_uint;
pub type EGLDisplay = *mut c_void;

pub type EGLConfig = *mut c_void;
pub type EGLSurface = *mut c_void;
pub type EGLContext = *mut c_void;
pub type __eglMustCastToProperFunctionPointerType = unsafe extern "C" fn();

pub const EGL_ALPHA_SIZE: EGLenum = 0x3021;
pub const EGL_BAD_ACCESS: EGLenum = 0x3002;
pub const EGL_BAD_ALLOC: EGLenum = 0x3003;
pub const EGL_BAD_ATTRIBUTE: EGLenum = 0x3004;
pub const EGL_BAD_CONFIG: EGLenum = 0x3005;
pub const EGL_BAD_CONTEXT: EGLenum = 0x3006;
pub const EGL_BAD_CURRENT_SURFACE: EGLenum = 0x3007;
pub const EGL_BAD_DISPLAY: EGLenum = 0x3008;
pub const EGL_BAD_MATCH: EGLenum = 0x3009;
pub const EGL_BAD_NATIVE_PIXMAP: EGLenum = 0x300A;
pub const EGL_BAD_NATIVE_WINDOW: EGLenum = 0x300B;
pub const EGL_BAD_PARAMETER: EGLenum = 0x300C;
pub const EGL_BAD_SURFACE: EGLenum = 0x300D;
pub const EGL_BLUE_SIZE: EGLenum = 0x3022;
pub const EGL_BUFFER_SIZE: EGLenum = 0x3020;
pub const EGL_CONFIG_CAVEAT: EGLenum = 0x3027;
pub const EGL_CONFIG_ID: EGLenum = 0x3028;
pub const EGL_CORE_NATIVE_ENGINE: EGLenum = 0x305B;
pub const EGL_DEPTH_SIZE: EGLenum = 0x3025;
pub const DONT_CARE: EGLint = -1;
pub const EGL_DRAW: EGLenum = 0x3059;
pub const EGL_EXTENSIONS: EGLenum = 0x3055;
pub const EGL_FALSE: EGLBoolean = 0;
pub const EGL_GREEN_SIZE: EGLenum = 0x3023;
pub const EGL_HEIGHT: EGLenum = 0x3056;
pub const EGL_LARGEST_PBUFFER: EGLenum = 0x3058;
pub const EGL_LEVEL: EGLenum = 0x3029;
pub const EGL_MAX_PBUFFER_HEIGHT: EGLenum = 0x302A;
pub const EGL_MAX_PBUFFER_PIXELS: EGLenum = 0x302B;
pub const EGL_MAX_PBUFFER_WIDTH: EGLenum = 0x302C;
pub const EGL_NATIVE_RENDERABLE: EGLenum = 0x302D;
pub const EGL_NATIVE_VISUAL_ID: EGLenum = 0x302E;
pub const EGL_NATIVE_VISUAL_TYPE: EGLenum = 0x302F;
pub const EGL_NONE: EGLenum = 0x3038;
pub const EGL_NON_CONFORMANT_CONFIG: EGLenum = 0x3051;
pub const EGL_NOT_INITIALIZED: EGLenum = 0x3001;
pub const EGL_NO_CONTEXT: EGLContext = null_mut();
pub const EGL_NO_DISPLAY: EGLDisplay = null_mut();
pub const EGL_NO_SURFACE: EGLSurface = null_mut();
pub const EGL_PBUFFER_BIT: EGLenum = 0x0001;
pub const EGL_PIXMAP_BIT: EGLenum = 0x0002;
pub const EGL_READ: EGLenum = 0x305A;
pub const EGL_RED_SIZE: EGLenum = 0x3024;
pub const EGL_SAMPLES: EGLenum = 0x3031;
pub const EGL_SAMPLE_BUFFERS: EGLenum = 0x3032;
pub const EGL_SLOW_CONFIG: EGLenum = 0x3050;
pub const EGL_STENCIL_SIZE: EGLenum = 0x3026;
pub const EGL_SUCCESS: EGLenum = 0x3000;
pub const EGL_SURFACE_TYPE: EGLenum = 0x3033;
pub const EGL_TRANSPARENT_BLUE_VALUE: EGLenum = 0x3035;
pub const EGL_TRANSPARENT_GREEN_VALUE: EGLenum = 0x3036;
pub const EGL_TRANSPARENT_RED_VALUE: EGLenum = 0x3037;
pub const EGL_TRANSPARENT_RGB: EGLenum = 0x3052;
pub const EGL_TRANSPARENT_TYPE: EGLenum = 0x3034;
pub const EGL_TRUE: EGLBoolean = 1;
pub const EGL_VENDOR: EGLenum = 0x3053;
pub const EGL_VERSION: EGLenum = 0x3054;
pub const EGL_WIDTH: EGLenum = 0x3057;
pub const EGL_WINDOW_BIT: EGLenum = 0x0004;

// 1.1

pub const EGL_BACK_BUFFER: EGLenum = 0x3084;
pub const EGL_BIND_TO_TEXTURE_RGB: EGLenum = 0x3039;
pub const EGL_BIND_TO_TEXTURE_RGBA: EGLenum = 0x303A;
pub const EGL_CONTEXT_LOST: EGLenum = 0x300E;
pub const EGL_MIN_SWAP_INTERVAL: EGLenum = 0x303B;
pub const EGL_MAX_SWAP_INTERVAL: EGLenum = 0x303C;
pub const EGL_MIPMAP_TEXTURE: EGLenum = 0x3082;
pub const EGL_MIPMAP_LEVEL: EGLenum = 0x3083;
pub const EGL_NO_TEXTURE: EGLenum = 0x305C;
pub const EGL_TEXTURE_2D: EGLenum = 0x305F;
pub const EGL_TEXTURE_FORMAT: EGLenum = 0x3080;
pub const EGL_TEXTURE_RGB: EGLenum = 0x305D;
pub const EGL_TEXTURE_RGBA: EGLenum = 0x305E;
pub const EGL_TEXTURE_TARGET: EGLenum = 0x3081;

// 1.2

pub type EGLenum = c_uint;
pub type EGLClientBuffer = *mut c_void;

pub const EGL_ALPHA_FORMAT: EGLenum = 0x3088;
pub const EGL_ALPHA_FORMAT_NONPRE: EGLenum = 0x308B;
pub const EGL_ALPHA_FORMAT_PRE: EGLenum = 0x308C;
pub const EGL_ALPHA_MASK_SIZE: EGLenum = 0x303E;
pub const EGL_BUFFER_PRESERVED: EGLenum = 0x3094;
pub const EGL_BUFFER_DESTROYED: EGLenum = 0x3095;
pub const EGL_CLIENT_APIS: EGLenum = 0x308D;
pub const EGL_COLORSPACE: EGLenum = 0x3087;
#[allow(non_upper_case_globals)]
pub const EGL_COLORSPACE_sRGB: EGLenum = 0x3089;
pub const EGL_COLORSPACE_LINEAR: EGLenum = 0x308A;
pub const EGL_COLOR_BUFFER_TYPE: EGLenum = 0x303F;
pub const EGL_CONTEXT_CLIENT_TYPE: EGLenum = 0x3097;
pub const EGL_DISPLAY_SCALING: EGLenum = 10000;
pub const EGL_HORIZONTAL_RESOLUTION: EGLenum = 0x3090;
pub const EGL_LUMINANCE_BUFFER: EGLenum = 0x308F;
pub const EGL_LUMINANCE_SIZE: EGLenum = 0x303D;
pub const EGL_OPENGL_ES_BIT: EGLenum = 0x0001;
pub const EGL_OPENVG_BIT: EGLenum = 0x0002;
pub const EGL_OPENGL_ES_API: EGLenum = 0x30A0;
pub const EGL_OPENVG_API: EGLenum = 0x30A1;
pub const EGL_OPENVG_IMAGE: EGLenum = 0x3096;
pub const EGL_PIXEL_ASPECT_RATIO: EGLenum = 0x3092;
pub const EGL_RENDERABLE_TYPE: EGLenum = 0x3040;
pub const EGL_RENDER_BUFFER: EGLenum = 0x3086;
pub const EGL_RGB_BUFFER: EGLenum = 0x308E;
pub const EGL_SINGLE_BUFFER: EGLenum = 0x3085;
pub const EGL_SWAP_BEHAVIOR: EGLenum = 0x3093;
pub const EGL_UNKNOWN: EGLint = -1;
pub const EGL_VERTICAL_RESOLUTION: EGLenum = 0x3091;

// 1.3

pub const EGL_CONFORMANT: EGLenum = 0x3042;
pub const EGL_CONTEXT_CLIENT_VERSION: EGLenum = 0x3098;
pub const EGL_MATCH_NATIVE_PIXMAP: EGLenum = 0x3041;
pub const EGL_OPENGL_ES2_BIT: EGLenum = 0x0004;
pub const EGL_VG_ALPHA_FORMAT: EGLenum = 0x3088;
pub const EGL_VG_ALPHA_FORMAT_NONPRE: EGLenum = 0x308B;
pub const EGL_VG_ALPHA_FORMAT_PRE: EGLenum = 0x308C;
pub const EGL_VG_ALPHA_FORMAT_PRE_BIT: EGLenum = 0x0040;
pub const EGL_VG_COLORSPACE: EGLenum = 0x3087;
#[allow(non_upper_case_globals)]
pub const EGL_VG_COLORSPACE_sRGB: EGLenum = 0x3089;
pub const EGL_VG_COLORSPACE_LINEAR: EGLenum = 0x308A;
pub const EGL_VG_COLORSPACE_LINEAR_BIT: EGLenum = 0x0020;

// 1.4

pub const EGL_DEFAULT_DISPLAY: EGLNativeDisplayType = null_mut();
pub const EGL_MULTISAMPLE_RESOLVE_BOX_BIT: EGLenum = 0x0200;
pub const EGL_MULTISAMPLE_RESOLVE: EGLenum = 0x3099;
pub const EGL_MULTISAMPLE_RESOLVE_DEFAULT: EGLenum = 0x309A;
pub const EGL_MULTISAMPLE_RESOLVE_BOX: EGLenum = 0x309B;
pub const EGL_OPENGL_API: EGLenum = 0x30A2;
pub const EGL_OPENGL_BIT: EGLenum = 0x0008;
pub const EGL_SWAP_BEHAVIOR_PRESERVED_BIT: EGLenum = 0x0400;

// 1.5

pub type EGLSync = *mut c_void;
pub type EGLAttrib = isize;
pub type EGLTime = khronos_utime_nanoseconds_t;
pub type EGLImage = *mut c_void;

pub const EGL_CONTEXT_MAJOR_VERSION: EGLenum = 0x3098;
pub const EGL_CONTEXT_MINOR_VERSION: EGLenum = 0x30FB;
pub const EGL_CONTEXT_OPENGL_PROFILE_MASK: EGLenum = 0x30FD;
pub const EGL_CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY: EGLenum = 0x31BD;
pub const EGL_NO_RESET_NOTIFICATION: EGLenum = 0x31BE;
pub const EGL_LOSE_CONTEXT_ON_RESET: EGLenum = 0x31BF;
pub const EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT: EGLenum = 0x00000001;
pub const EGL_CONTEXT_OPENGL_COMPATIBILITY_PROFILE_BIT: EGLenum = 0x00000002;
pub const EGL_CONTEXT_OPENGL_DEBUG: EGLenum = 0x31B0;
pub const EGL_CONTEXT_OPENGL_FORWARD_COMPATIBLE: EGLenum = 0x31B1;
pub const EGL_CONTEXT_OPENGL_ROBUST_ACCESS: EGLenum = 0x31B2;
pub const EGL_OPENGL_ES3_BIT: EGLenum = 0x00000040;
pub const EGL_CL_EVENT_HANDLE: EGLenum = 0x309C;
pub const EGL_SYNC_CL_EVENT: EGLenum = 0x30FE;
pub const EGL_SYNC_CL_EVENT_COMPLETE: EGLenum = 0x30FF;
pub const EGL_SYNC_PRIOR_COMMANDS_COMPLETE: EGLenum = 0x30F0;
pub const EGL_SYNC_TYPE: EGLenum = 0x30F7;
pub const EGL_SYNC_STATUS: EGLenum = 0x30F1;
pub const EGL_SYNC_CONDITION: EGLenum = 0x30F8;
pub const EGL_SIGNALED: EGLenum = 0x30F2;
pub const EGL_UNSIGNALED: EGLenum = 0x30F3;
pub const EGL_SYNC_FLUSH_COMMANDS_BIT: EGLenum = 0x0001;
pub const EGL_FOREVER: u64 = 0xFFFFFFFFFFFFFFFF;
pub const EGL_TIMEOUT_EXPIRED: EGLenum = 0x30F5;
pub const EGL_CONDITION_SATISFIED: EGLenum = 0x30F6;
pub const NO_SYNC: EGLSync = null_mut();
pub const EGL_SYNC_FENCE: EGLenum = 0x30F9;
pub const EGL_GL_COLORSPACE: EGLenum = 0x309D;
pub const EGL_GL_COLORSPACE_SRGB: EGLenum = 0x3089;
pub const EGL_GL_COLORSPACE_LINEAR: EGLenum = 0x308A;
pub const EGL_GL_RENDERBUFFER: EGLenum = 0x30B9;
pub const EGL_GL_TEXTURE_2D: EGLenum = 0x30B1;
pub const EGL_GL_TEXTURE_LEVEL: EGLenum = 0x30BC;
pub const EGL_GL_TEXTURE_3D: EGLenum = 0x30B2;
pub const EGL_GL_TEXTURE_ZOFFSET: EGLenum = 0x30BD;
pub const EGL_GL_TEXTURE_CUBE_MAP_POSITIVE_X: EGLenum = 0x30B3;
pub const EGL_GL_TEXTURE_CUBE_MAP_NEGATIVE_X: EGLenum = 0x30B4;
pub const EGL_GL_TEXTURE_CUBE_MAP_POSITIVE_Y: EGLenum = 0x30B5;
pub const EGL_GL_TEXTURE_CUBE_MAP_NEGATIVE_Y: EGLenum = 0x30B6;
pub const EGL_GL_TEXTURE_CUBE_MAP_POSITIVE_Z: EGLenum = 0x30B7;
pub const EGL_GL_TEXTURE_CUBE_MAP_NEGATIVE_Z: EGLenum = 0x30B8;
pub const EGL_IMAGE_PRESERVED: EGLenum = 0x30D2;
pub const EGL_NO_IMAGE: EGLImage = null_mut();

pub struct Lib {
    pub eglChooseConfig: unsafe extern "C" fn(
        dpy: EGLDisplay,
        attrib_list: *const EGLint,
        configs: *mut EGLConfig,
        config_size: EGLint,
        num_config: *mut EGLint,
    ) -> EGLBoolean,
    pub eglCopyBuffers: unsafe extern "C" fn(
        dpy: EGLDisplay,
        surface: EGLSurface,
        target: EGLNativePixmapType,
    ) -> EGLBoolean,
    pub eglCreateContext: unsafe extern "C" fn(
        dpy: EGLDisplay,
        config: EGLConfig,
        share_context: EGLContext,
        attrib_list: *const EGLint,
    ) -> EGLContext,
    pub eglCreatePbufferSurface: unsafe extern "C" fn(
        dpy: EGLDisplay,
        config: EGLConfig,
        attrib_list: *const EGLint,
    ) -> EGLSurface,
    pub eglCreatePixmapSurface: unsafe extern "C" fn(
        dpy: EGLDisplay,
        config: EGLConfig,
        pixmap: EGLNativePixmapType,
        attrib_list: *const EGLint,
    ) -> EGLSurface,
    pub eglCreateWindowSurface: unsafe extern "C" fn(
        dpy: EGLDisplay,
        config: EGLConfig,
        win: EGLNativeWindowType,
        attrib_list: *const EGLint,
    ) -> EGLSurface,
    pub eglDestroyContext: unsafe extern "C" fn(dpy: EGLDisplay, ctx: EGLContext) -> EGLBoolean,
    pub eglDestroySurface: unsafe extern "C" fn(dpy: EGLDisplay, surface: EGLSurface) -> EGLBoolean,
    pub eglGetConfigAttrib: unsafe extern "C" fn(
        dpy: EGLDisplay,
        config: EGLConfig,
        attribute: EGLint,
        value: *mut EGLint,
    ) -> EGLBoolean,
    pub eglGetConfigs: unsafe extern "C" fn(
        dpy: EGLDisplay,
        configs: *mut EGLConfig,
        config_size: EGLint,
        num_config: *mut EGLint,
    ) -> EGLBoolean,
    pub eglGetCurrentDisplay: unsafe extern "C" fn() -> EGLDisplay,
    pub eglGetCurrentSurface: unsafe extern "C" fn(readdraw: EGLint) -> EGLSurface,
    pub eglGetDisplay: unsafe extern "C" fn(display_id: EGLNativeDisplayType) -> EGLDisplay,
    pub eglGetError: unsafe extern "C" fn() -> EGLint,
    pub eglGetProcAddress:
        unsafe extern "C" fn(procname: *const char) -> __eglMustCastToProperFunctionPointerType,
    pub eglInitialize:
        unsafe extern "C" fn(dpy: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> EGLBoolean,
    pub eglMakeCurrent: unsafe extern "C" fn(
        dpy: EGLDisplay,
        draw: EGLSurface,
        read: EGLSurface,
        ctx: EGLContext,
    ) -> EGLBoolean,
    pub eglQueryContext: unsafe extern "C" fn(
        dpy: EGLDisplay,
        ctx: EGLContext,
        attribute: EGLint,
        value: *mut EGLint,
    ) -> EGLBoolean,
    pub eglQueryString: unsafe extern "C" fn(dpy: EGLDisplay, name: EGLint) -> *const c_char,
    pub eglQuerySurface: unsafe extern "C" fn(
        dpy: EGLDisplay,
        surface: EGLSurface,
        attribute: EGLint,
        value: *mut EGLint,
    ) -> EGLBoolean,
    pub eglSwapBuffers: unsafe extern "C" fn(dpy: EGLDisplay, surface: EGLSurface) -> EGLBoolean,
    pub eglTerminate: unsafe extern "C" fn(dpy: EGLDisplay) -> EGLBoolean,
    pub eglWaitGL: unsafe extern "C" fn() -> EGLBoolean,
    pub eglWaitNative: unsafe extern "C" fn(engine: EGLint) -> EGLBoolean,

    // 1.1
    pub eglBindTexImage:
        unsafe extern "C" fn(dpy: EGLDisplay, surface: EGLSurface, buffer: EGLint) -> EGLBoolean,
    pub eglReleaseTexImage:
        unsafe extern "C" fn(dpy: EGLDisplay, surface: EGLSurface, buffer: EGLint) -> EGLBoolean,
    pub eglSurfaceAttrib: unsafe extern "C" fn(
        dpy: EGLDisplay,
        surface: EGLSurface,
        attribute: EGLint,
        value: EGLint,
    ) -> EGLBoolean,
    pub eglSwapInterval: unsafe extern "C" fn(dpy: EGLDisplay, interval: EGLint) -> EGLBoolean,

    // 1.2
    pub eglBindAPI: unsafe extern "C" fn(api: EGLenum) -> EGLBoolean,
    pub eglQueryAPI: unsafe extern "C" fn() -> EGLenum,
    pub eglCreatePbufferFromClientBuffer: unsafe extern "C" fn(
        dpy: EGLDisplay,
        buftype: EGLenum,
        buffer: EGLClientBuffer,
        config: EGLConfig,
        attrib_list: *const EGLint,
    ) -> EGLSurface,
    pub eglReleaseThread: unsafe extern "C" fn() -> EGLBoolean,
    pub eglWaitClient: unsafe extern "C" fn() -> EGLBoolean,

    // 1.4
    pub eglGetCurrentContext: unsafe extern "C" fn() -> EGLContext,

    // 1.5
    pub eglCreateSync: unsafe extern "C" fn(
        dpy: EGLDisplay,
        r#type: EGLenum,
        attrib_list: *const EGLAttrib,
    ) -> EGLSync,
    pub eglDestroySync: unsafe extern "C" fn(dpy: EGLDisplay, sync: EGLSync) -> EGLBoolean,
    pub eglClientWaitSync: unsafe extern "C" fn(
        dpy: EGLDisplay,
        sync: EGLSync,
        flags: EGLint,
        timeout: EGLTime,
    ) -> EGLint,
    pub eglGetSyncAttrib: unsafe extern "C" fn(
        dpy: EGLDisplay,
        sync: EGLSync,
        attribute: EGLint,
        value: *mut EGLAttrib,
    ) -> EGLBoolean,
    pub eglCreateImage: unsafe extern "C" fn(
        dpy: EGLDisplay,
        ctx: EGLContext,
        target: EGLenum,
        buffer: EGLClientBuffer,
        attrib_list: *const EGLAttrib,
    ) -> EGLImage,
    pub eglDestroyImage: unsafe extern "C" fn(dpy: EGLDisplay, image: EGLImage) -> EGLBoolean,
    pub eglGetPlatformDisplay: unsafe extern "C" fn(
        platform: EGLenum,
        native_display: *mut c_void,
        attrib_list: *const EGLAttrib,
    ) -> EGLDisplay,
    pub eglCreatePlatformWindowSurface: unsafe extern "C" fn(
        dpy: EGLDisplay,
        config: EGLConfig,
        native_window: *mut c_void,
        attrib_list: *const EGLAttrib,
    ) -> EGLSurface,
    pub eglCreatePlatformPixmapSurface: unsafe extern "C" fn(
        dpy: EGLDisplay,
        config: EGLConfig,
        native_pixmap: *mut c_void,
        attrib_list: *const EGLAttrib,
    ) -> EGLSurface,
    pub eglWaitSync:
        unsafe extern "C" fn(dpy: EGLDisplay, sync: EGLSync, flags: EGLint) -> EGLBoolean,

    _dl: DynLib,
}

impl Lib {
    pub fn load() -> anyhow::Result<Self> {
        let dl = DynLib::open(c"libEGL.so").or_else(|_| DynLib::open(c"libEGL.so.1"))?;

        Ok(Self {
            eglChooseConfig: dl.lookup(c"eglChooseConfig")?,
            eglCopyBuffers: dl.lookup(c"eglCopyBuffers")?,
            eglCreateContext: dl.lookup(c"eglCreateContext")?,
            eglCreatePbufferSurface: dl.lookup(c"eglCreatePbufferSurface")?,
            eglCreatePixmapSurface: dl.lookup(c"eglCreatePixmapSurface")?,
            eglCreateWindowSurface: dl.lookup(c"eglCreateWindowSurface")?,
            eglDestroyContext: dl.lookup(c"eglDestroyContext")?,
            eglDestroySurface: dl.lookup(c"eglDestroySurface")?,
            eglGetConfigAttrib: dl.lookup(c"eglGetConfigAttrib")?,
            eglGetConfigs: dl.lookup(c"eglGetConfigs")?,
            eglGetCurrentDisplay: dl.lookup(c"eglGetCurrentDisplay")?,
            eglGetCurrentSurface: dl.lookup(c"eglGetCurrentSurface")?,
            eglGetDisplay: dl.lookup(c"eglGetDisplay")?,
            eglGetError: dl.lookup(c"eglGetError")?,
            eglGetProcAddress: dl.lookup(c"eglGetProcAddress")?,
            eglInitialize: dl.lookup(c"eglInitialize")?,
            eglMakeCurrent: dl.lookup(c"eglMakeCurrent")?,
            eglQueryContext: dl.lookup(c"eglQueryContext")?,
            eglQueryString: dl.lookup(c"eglQueryString")?,
            eglQuerySurface: dl.lookup(c"eglQuerySurface")?,
            eglSwapBuffers: dl.lookup(c"eglSwapBuffers")?,
            eglTerminate: dl.lookup(c"eglTerminate")?,
            eglWaitGL: dl.lookup(c"eglWaitGL")?,
            eglWaitNative: dl.lookup(c"eglWaitNative")?,

            // 1.1
            eglBindTexImage: dl.lookup(c"eglBindTexImage")?,
            eglReleaseTexImage: dl.lookup(c"eglReleaseTexImage")?,
            eglSurfaceAttrib: dl.lookup(c"eglSurfaceAttrib")?,
            eglSwapInterval: dl.lookup(c"eglSwapInterval")?,

            // 1.2
            eglBindAPI: dl.lookup(c"eglBindAPI")?,
            eglQueryAPI: dl.lookup(c"eglQueryAPI")?,
            eglCreatePbufferFromClientBuffer: dl.lookup(c"eglCreatePbufferFromClientBuffer")?,
            eglReleaseThread: dl.lookup(c"eglReleaseThread")?,
            eglWaitClient: dl.lookup(c"eglWaitClient")?,

            // 1.4
            eglGetCurrentContext: dl.lookup(c"eglGetCurrentContext")?,

            // 1.5
            eglCreateSync: dl.lookup(c"eglCreateSync")?,
            eglDestroySync: dl.lookup(c"eglDestroySync")?,
            eglClientWaitSync: dl.lookup(c"eglClientWaitSync")?,
            eglGetSyncAttrib: dl.lookup(c"eglGetSyncAttrib")?,
            eglCreateImage: dl.lookup(c"eglCreateImage")?,
            eglDestroyImage: dl.lookup(c"eglDestroyImage")?,
            eglGetPlatformDisplay: dl.lookup(c"eglGetPlatformDisplay")?,
            eglCreatePlatformWindowSurface: dl.lookup(c"eglCreatePlatformWindowSurface")?,
            eglCreatePlatformPixmapSurface: dl.lookup(c"eglCreatePlatformPixmapSurface")?,
            eglWaitSync: dl.lookup(c"eglWaitSync")?,

            _dl: dl,
        })
    }
}
