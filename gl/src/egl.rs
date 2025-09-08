use std::ops::Deref;

use dynlib::DynLib;

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/egl_types_generated.rs"));
    include!(concat!(env!("OUT_DIR"), "/egl_enums_generated.rs"));
    include!(concat!(env!("OUT_DIR"), "/egl_api_generated.rs"));
}

pub use generated::*;

pub struct EglApi {
    api: Api,
    _dynlib: DynLib,
}

impl Deref for EglApi {
    type Target = Api;

    fn deref(&self) -> &Self::Target {
        &self.api
    }
}

impl EglApi {
    pub fn load() -> Result<Self, dynlib::Error> {
        let dynlib = DynLib::load(c"libEGL.so").or_else(|_| DynLib::load(c"libEGL.so.1"))?;

        // NOTE: it seems like some funcs (the ones that get enabled by extensions (for example
        // EGL_KHR_image)) cannot be loaded with dlsym, but only with eglGetProcAddress.
        let get_proc_address =
            dynlib.lookup::<unsafe extern "C" fn(
                *const std::ffi::c_char,
            ) -> __eglMustCastToProperFunctionPointerType>(
                c"eglGetProcAddress"
            )?;

        let api = unsafe { Api::load_with(|name| get_proc_address(name) as _) };

        Ok(Self {
            api,
            _dynlib: dynlib,
        })
    }
}

// idk if i should keep this \/ here.

// NOTE: the stuff below doesn't care about deinitialization. os will gc xd.

// use std::{ffi::c_void, ops::Deref, ptr::null};
//
// use anyhow::anyhow;
// use dynlib::DynLib;
// use raw_window_handle as rwh;
//
// enum EglDisplayError {
//     SymNotAvailable,
//     NoDisplay,
// }
//
// pub enum EglDisplay {
//     /// eglGetPlatformDisplay
//     Khr(EGLDisplay),
//     /// eglGetPlatformDisplayEXT
//     /// - https://registry.khronos.org/EGL/extensions/EXT/EGL_EXT_platform_base.txt
//     Ext(EGLDisplay),
//     /// eglGetDisplay
//     /// > the set of platforms to which display_id is permitted to belong, as well as the actual
//     /// type of display_id, are implementation-specific.
//     /// - https://registry.khronos.org/EGL/sdk/docs/man/html/eglGetDisplay.xhtml
//     Uncertain(EGLDisplay),
// }
//
// impl EglDisplay {
//     pub fn get_platform_display(
//         api: &EglApi,
//         platform: EGLenum,
//         native_display: *mut c_void,
//         attribs: Option<&[EGLAttrib]>,
//     ) -> Result<EGLDisplay> {
//         if api.GetPlatformDisplay.as_ptr().is_null() {
//             return None;
//         }
//         let ret = unsafe {
//             api.GetPlatformDisplay(
//                 platform,
//                 native_display,
//                 attribs.map_or_else(|| null(), |attribs| attribs.as_ptr()),
//             )
//         };
//         if ret == NO_DISPLAY { None } else { Some(ret) }
//     }
//
//     pub fn get_platform_display_ext(
//         api: &EglApi,
//         platform: EGLenum,
//         native_display: *mut c_void,
//         attribs: Option<&[EGLint]>,
//     ) -> Option<EGLDisplay> {
//         if api.GetPlatformDisplayEXT.as_ptr().is_null() {
//             return None;
//         }
//         let ret = unsafe {
//             api.GetPlatformDisplayEXT(
//                 platform,
//                 native_display,
//                 attribs.map_or_else(|| null(), |attribs| attribs.as_ptr()),
//             )
//         };
//         if ret == NO_DISPLAY { None } else { Some(ret) }
//     }
//
//     pub fn get_display(api: &EglApi, native_display: *mut c_void) -> Option<EGLDisplay> {
//         let ret = unsafe { api.GetDisplay(native_display) };
//         if ret == NO_DISPLAY { None } else { Some(ret) }
//     }
//
//     pub fn from_wayland_display(
//         api: &EglApi,
//         wl_display: *mut c_void,
//         attribs: &[EGLAttrib],
//     ) -> anyhow::Result<Self> {
//         // Self::get_platform_display(api, PLATFORM_WAYLAND_KHR, wl_display, attribs);
//
//         todo!()
//     }
//
//     pub fn from_display_handle(api: &EglApi, handle: rwh::DisplayHandle) -> anyhow::Result<Self> {
//         match handle.as_raw() {
//             rwh::RawDisplayHandle::Wayland(handle) => {
//                 Self::from_wayland_display(api, handle.display.as_ptr())
//             }
//             _ => Err(anyhow!("unsupported window handle: {handle:?}")),
//         }
//     }
// }
