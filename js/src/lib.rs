use std::{error, fmt};

mod sys {
    use std::alloc::{Layout, alloc};

    use super::Handle;

    // TODO: don't use usize. be specific and strict.

    unsafe extern "C" {
        pub fn throw_str(ptr: *const u8, len: usize) -> !;

        pub fn string_new(ptr: *const u8, len: usize) -> Handle;
        pub fn number_new(value: f64) -> Handle;
        pub fn closure_new(call_by_ptr: extern "C" fn(ptr: *mut ()), ptr: *mut ()) -> Handle;

        pub fn increment_strong_count(handle: Handle);
        pub fn decrement_strong_count(handle: Handle);

        pub fn is_object(handle: Handle) -> bool;
        pub fn is_function(handle: Handle) -> bool;
        pub fn is_number(handle: Handle) -> bool;
        pub fn is_string(handle: Handle) -> bool;

        pub fn get(handle: Handle, prop_ptr: *const u8, prop_len: usize) -> Handle;
        pub fn set(handle: Handle, prop_ptr: *const u8, prop_len: usize, value_handle: Handle);
        pub fn call(
            handle: Handle,
            agrs_ptr: *const Handle,
            args_len: usize,
            ret_handle_ptr: *mut Handle,
        ) -> bool;

        pub fn string_get(handle: Handle, ptr: *mut u32, len: *mut u32);
        pub fn number_get(handle: Handle) -> f64;
    }

    #[unsafe(no_mangle)]
    extern "C" fn malloc(size: u32, align: u32) -> *mut u8 {
        let Ok(layout) = Layout::from_size_align(size as usize, align as usize) else {
            // TODO: should this be handled better somehow?
            const INVALID_LAYOUT: &str = "invalid layout";
            unsafe { throw_str(INVALID_LAYOUT.as_ptr(), INVALID_LAYOUT.len()) };
        };
        unsafe { alloc(layout) }
    }
}

pub use sys::throw_str;

// Handle is used to identify a js value, since the value itself can not be passed to wasm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct Handle(u32);

// Value represents a js value.
#[derive(Debug)]
#[repr(transparent)]
pub struct Value {
    handle: Handle,
}

// TODO: Error needs to be better.
#[derive(Debug, Clone)]
pub struct Error {
    value: Value,
}

pub struct Closure<F: ?Sized> {
    _f: Box<F>,
    value: Value,
}

// ----
// value

impl Clone for Value {
    fn clone(&self) -> Self {
        unsafe { sys::increment_strong_count(self.handle) };
        Self {
            handle: self.handle,
        }
    }
}

impl Drop for Value {
    fn drop(&mut self) {
        unsafe { sys::decrement_strong_count(self.handle) };
    }
}

impl Value {
    /// NOTE: the string is copied to the js heap and will be owned by the js garbage collector.
    pub fn from_str(value: &str) -> Self {
        let handle = unsafe { sys::string_new(value.as_ptr(), value.len()) };
        Self { handle }
    }

    /// NOTE: js number is a 64-bit ieee 754 value.
    pub fn from_f64(value: f64) -> Self {
        let handle = unsafe { sys::number_new(value) };
        Self { handle }
    }

    /// NOTE: the closure needs to be kept alive. be careful when using it as a callback with
    /// things like requestAnimationFrame, etc.
    pub fn from_closure<F: ?Sized>(closure: &Closure<F>) -> Self {
        closure.value.clone()
    }

    // ----

    pub fn is_object(&self) -> bool {
        unsafe { sys::is_object(self.handle) }
    }

    pub fn is_function(&self) -> bool {
        unsafe { sys::is_function(self.handle) }
    }

    pub fn is_number(&self) -> bool {
        unsafe { sys::is_number(self.handle) }
    }

    pub fn is_string(&self) -> bool {
        unsafe { sys::is_string(self.handle) }
    }

    // ----

    pub fn get(&self, p: &str) -> Value {
        assert!(self.is_object());
        let handle = unsafe { sys::get(self.handle, p.as_ptr(), p.len()) };
        Self { handle }
    }

    pub fn set(&self, p: &str, value: &Self) {
        assert!(self.is_object());
        unsafe { sys::set(self.handle, p.as_ptr(), p.len(), value.handle) }
    }

    pub fn call(&self, args: &[Self]) -> Result<Value, Error> {
        assert!(self.is_function());
        let mut ret = Value {
            handle: Handle(u32::MAX),
        };
        let ok = unsafe {
            sys::call(
                self.handle,
                // NOTE: ok to cast; Value wraps Handle and nothing else.
                args.as_ptr().cast(),
                args.len(),
                &mut ret.handle,
            )
        };
        if ok {
            Ok(ret)
        } else {
            Err(Error { value: ret })
        }
    }

    // ----

    pub fn try_as_f64(&self) -> Option<f64> {
        if self.is_number() {
            Some(unsafe { sys::number_get(self.handle) })
        } else {
            None
        }
    }

    pub fn as_f64(&self) -> f64 {
        self.try_as_f64().expect("value is number")
    }

    pub fn try_as_string(&self) -> Option<String> {
        if self.is_string() {
            let mut ptr: u32 = u32::MAX;
            let mut len: u32 = u32::MAX;
            unsafe { sys::string_get(self.handle, &mut ptr, &mut len) };
            assert!(ptr != u32::MAX && len != u32::MAX);
            let buf = unsafe { Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize) };
            String::from_utf8(buf).ok()
        } else {
            None
        }
    }

    pub fn as_string(&self) -> String {
        self.try_as_string().expect("value is string")
    }
}

// ----
// error

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = self.value.get("message");
        if let Some(s) = message.try_as_string() {
            f.write_str(s.as_str())
        } else {
            f.write_str("could not get error message")
        }
    }
}

// ----
// closure

impl Closure<dyn FnMut()> {
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        #[inline(never)]
        extern "C" fn call_by_ptr<F>(ptr: *mut ())
        where
            F: FnMut() + 'static,
        {
            assert!(!ptr.is_null());
            let f: &mut F = unsafe { &mut *(ptr as *mut F) };
            f();
        }

        let mut f = Box::new(f);
        let handle = unsafe { sys::closure_new(call_by_ptr::<F>, &raw mut *f as *mut ()) };

        Self {
            _f: f,
            value: Value { handle },
        }
    }
}

// ----
// predefines

// NOTE: handles must much predefined handles in js glue code.
const UNDEFINED: Handle = Handle(0);
const NULL: Handle = Handle(1);
const GLOBAL: Handle = Handle(2);
const GLUE: Handle = Handle(3);

fn predefine(handle: Handle) -> Value {
    unsafe { sys::increment_strong_count(handle) };
    Value { handle }
}

pub fn undefined() -> Value {
    predefine(UNDEFINED)
}

pub fn null() -> Value {
    predefine(NULL)
}

pub fn global() -> Value {
    predefine(GLOBAL)
}

pub fn glue() -> Value {
    predefine(GLUE)
}
