use std::ffi::{CStr, CString, c_void};
use std::mem::transmute_copy;
use std::ptr::NonNull;

use anyhow::anyhow;
use libc::{dlclose, dlerror, dlopen, dlsym};

pub struct DynLib(NonNull<c_void>);

impl DynLib {
    pub fn open(filename: &CStr) -> anyhow::Result<Self> {
        unsafe {
            let handle = dlopen(filename.as_ptr(), libc::RTLD_LAZY);

            if handle.is_null() {
                Err(anyhow!(
                    CString::from_raw(dlerror())
                        .into_string()
                        .unwrap_or("invalid dlerror string".to_string())
                ))
            } else {
                Ok(Self(NonNull::new_unchecked(handle)))
            }
        }
    }

    pub fn lookup<F: Sized>(&self, name: &CStr) -> anyhow::Result<F> {
        unsafe {
            _ = dlerror();

            let addr = dlsym(self.0.as_ptr(), name.as_ptr());

            let err = dlerror();
            if !err.is_null() {
                Err(anyhow!(
                    CString::from_raw(err)
                        .into_string()
                        .unwrap_or("invalid dlerror string".to_string())
                ))
            } else {
                Ok(transmute_copy(&addr))
            }
        }
    }
}

impl Drop for DynLib {
    fn drop(&mut self) {
        unsafe {
            dlclose(self.0.as_ptr());
        }
    }
}

#[macro_export]
macro_rules! opaque_struct {
    ($name:ident) => {
        #[repr(C)]
        pub struct $name {
            _data: [u8; 0],
            _marker: std::marker::PhantomData<(*mut u8, std::marker::PhantomPinned)>,
        }
    };
}
