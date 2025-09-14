use std::ops::Deref;

use dynlib::DynLib;

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
mod types {
    include!(concat!(env!("OUT_DIR"), "/egl_types_generated.rs"));
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
mod enums {
    use super::types::*;

    include!(concat!(env!("OUT_DIR"), "/egl_enums_generated.rs"));
}

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
mod api {
    use super::types::*;

    include!(concat!(env!("OUT_DIR"), "/egl_api_generated.rs"));
}

pub use enums::*;
pub use types::*;

pub struct Api {
    api: api::Api,
    _dynlib: DynLib,
}

impl Deref for Api {
    type Target = api::Api;

    fn deref(&self) -> &Self::Target {
        &self.api
    }
}

impl Api {
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

        let api = unsafe { api::Api::load_with(|name| get_proc_address(name) as _) };

        Ok(Self {
            api,
            _dynlib: dynlib,
        })
    }
}
