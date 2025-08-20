use std::{ffi::CStr, ops::Deref, ptr::null_mut};

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

// TODO: consider naming this LibEgl.
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
