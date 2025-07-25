use std::ffi::{CStr, CString, c_void};
use std::mem;
use std::ptr::NonNull;
use std::{error, fmt};

use libc::{dlclose, dlerror, dlopen, dlsym};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(Option<CString>);

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(ref description) => f.write_fmt(format_args!("{:?}", description)),
            None => f.write_str("fucky wacky, no error was reported"),
        }
    }
}

impl Error {
    fn from_dlerror() -> Self {
        let err = unsafe { dlerror() };
        if err.is_null() {
            Self(None)
        } else {
            Self(Some(CString::from(unsafe { CStr::from_ptr(err) })))
        }
    }
}

pub struct DynLib(NonNull<c_void>);

impl DynLib {
    pub fn load(filename: &CStr) -> Result<Self, Error> {
        if let Some(handle) = NonNull::new(unsafe { dlopen(filename.as_ptr(), libc::RTLD_LAZY) }) {
            Ok(Self(handle))
        } else {
            Err(Error::from_dlerror())
        }
    }

    pub fn lookup<F: Sized>(&self, name: &CStr) -> Result<F, Error> {
        assert_eq!(mem::size_of::<F>(), mem::size_of::<usize>());
        let addr = unsafe { dlsym(self.0.as_ptr(), name.as_ptr()) };
        if addr.is_null() {
            Err(Error::from_dlerror())
        } else {
            Ok(unsafe { mem::transmute_copy(&addr) })
        }
    }
}

impl Drop for DynLib {
    fn drop(&mut self) {
        unsafe { dlclose(self.0.as_ptr()) };
    }
}
