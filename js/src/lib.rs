use std::{error, fmt};

// NOTE: this is based on  <https://pkg.go.dev/syscall/js>.
// it's nothing like wasm-bindgen.

// TODO: get and set must operate not on singular properties, but on property paths.
//   instead of GLOBAL.get("performance").get("now")
//   must be possible to do GLOBAL.get(&["performance", "now"])

// TODO: call and construct must operate not on owned Value, but on references.
//   this will elliminate many ref counting calls.

// TODO: owned Value is not always good; there needs to be some kind of weak handle.
//   gl::wrap Adapter implementor owns Value and hands out self-made weak references, while it's a
//   functional solution, it's somewhat ugly.

mod sys {
    use std::alloc;

    // NOTE: this is an alias to u64, and not super::Value because Value does ref counting in its
    // Copy and Drop.
    // instead we're operating on Value's underlying value (xd) which is u64;
    // but it is semantically a bit confusing to refer to it as u64 thus this:
    type Value = u64;

    unsafe extern "C" {
        // there are cases when Value is treated as a (certainly) ref which means it's not
        // predefined nor it is a number.
        //
        // in cases when Value passed as a pointer - it can be anything;
        // Value wraps u64, rust's u64 compiles to js BigInt and in js there's no easy way to convert
        // between bin reprs of f64 (because in js any number is an ieee 754 float 64) and BigInt.
        //
        // but it's easy with the DataView thing that operates on byte offsets (wasm's stack and
        // heap are linear; both exist within the same memory object).

        pub fn throw_str(ptr: *const u8, len: u32) -> !;

        pub fn string_new(ptr: *const u8, len: u32, out: *mut Value);
        pub fn closure_new(call_by_ptr: extern "C" fn(ptr: *mut ()), ptr: *mut (), out: *mut Value);

        // TODO: can ref counting be done on rust side to avoid roundtrips when cloning refs?
        pub fn increment_ref_count(r#ref: Value);
        pub fn decrement_ref_count(r#ref: Value);

        pub fn get(r#ref: Value, prop_ptr: *const u8, prop_len: u32, out: *mut Value);
        // NOTE: value arg is a pointer because the value itself can be a number or predefined or a
        // reference.
        pub fn set(r#ref: Value, prop_ptr: *const u8, prop_len: u32, value: *const Value);

        pub fn call(r#ref: Value, agrs_ptr: *const Value, args_len: u32, out: *mut Value) -> bool;
        // NOTE: some things require you to invoke their constructor (for example Uint8Array).
        //   this is almost the exact copy of the `call` method.
        pub fn construct(
            r#ref: Value,
            agrs_ptr: *const Value,
            args_len: u32,
            out: *mut Value,
        ) -> bool;

        pub fn string_get(r#ref: Value, ptr: *mut u32, len: *mut u32);
    }

    // NOTE: this allows js to allocate memory that then can be handed-off to rust.
    #[unsafe(no_mangle)]
    extern "C" fn alloc(size: u32, align: u32) -> *mut u8 {
        let Ok(layout) = alloc::Layout::from_size_align(size as usize, align as usize) else {
            // TODO: should this be handled better somehow?
            super::throw_str("invalid layout");
        };
        unsafe { alloc::alloc(layout) }
    }
}

pub fn throw_str(s: &str) -> ! {
    unsafe { sys::throw_str(s.as_ptr(), s.len() as u32) }
}

// ----
// value

// Value is a nan-tagged thingie. it can represent ieee 754 float 64; it can be predefined; it can
// carry a reference to something that js owns.
#[derive(Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Value(u64);

impl Clone for Value {
    fn clone(&self) -> Self {
        if self.is_ref() {
            unsafe { sys::increment_ref_count(self.0) };
        }
        Self(self.0)
    }
}

impl Drop for Value {
    fn drop(&mut self) {
        if self.is_ref() {
            unsafe { sys::decrement_ref_count(self.0) };
        }
    }
}

// nan-tagging:
//   - https://craftinginterpreters.com/optimization.html#nan-boxing
//   - https://anniecherkaev.com/the-secret-life-of-nan
//   - https://wingolog.org/archives/2011/05/18/value-representation-in-javascript-implementations
//
// additionally theres this (similar tecnique can be applied to real pointers):
//   - https://source.android.com/docs/security/test/tagged-pointers
//   - https://fedang.net/posts/pointer-tagging/

const QUIET_NAN: u64 = 0x7ff8_0000_0000_0000;
const _: () = assert!(f64::NAN.to_bits() == QUIET_NAN);
const TY_MASK: u64 = (1 << 8) - 1;
const ID_MASK: u64 = (1 << 32) - 1;

// NOTE: TY_NUMBER does not exist because the whole thing is either a number or anything else.
const TY_DONT_CARE: u64 = 0;
const TY_OBJECT: u64 = 1;
const TY_FUNCTION: u64 = 2;
const TY_STRING: u64 = 3;

// NOTE: ids can't start at 0 because when encoded into tagged/boxed nan there would be no
// distinction between id 0 and nan.
const ID_UNDEFINED: u64 = 1;
const ID_NULL: u64 = 2;
const ID_NAN: u64 = 3;
const ID_TRUE: u64 = 4;
const ID_FALSE: u64 = 5;
const ID_GLOBAL: u64 = 6;
const ID_GLUE: u64 = 7;
const ID_MAX: u64 = 8;

pub const UNDEFINED: Value = Value::from_ty_id(TY_DONT_CARE, ID_UNDEFINED);
pub const NULL: Value = Value::from_ty_id(TY_DONT_CARE, ID_NULL);
pub const NAN: Value = Value::from_ty_id(TY_DONT_CARE, ID_NAN);
pub const TRUE: Value = Value::from_ty_id(TY_DONT_CARE, ID_TRUE);
pub const FALSE: Value = Value::from_ty_id(TY_DONT_CARE, ID_FALSE);
pub const GLOBAL: Value = Value::from_ty_id(TY_OBJECT, ID_GLOBAL);
pub const GLUE: Value = Value::from_ty_id(TY_OBJECT, ID_GLUE);

impl Value {
    const fn from_ty_id(ty: u64, id: u64) -> Self {
        Self(QUIET_NAN | (ty << 32) | id)
    }

    /// js number is a 64-bit ieee 754 value.
    pub const fn from_f64(f: f64) -> Self {
        if f.is_nan() {
            Self::from_ty_id(TY_DONT_CARE, ID_NAN)
        } else {
            Self(f.to_bits())
        }
    }

    /// the string is copied to the js heap and will be owned by the js garbage collector.
    pub fn from_str(s: &str) -> Self {
        let mut ret = UNDEFINED;
        unsafe { sys::string_new(s.as_ptr(), s.len() as u32, &mut ret.0) };
        ret
    }

    /// the closure needs to be kept alive.
    /// be careful when using it as a callback with things like requestAnimationFrame, etc.
    pub fn from_closure<F: ?Sized>(c: &Closure<F>) -> Self {
        c.value.clone()
    }

    pub fn from_bool(b: bool) -> Self {
        if b { TRUE } else { FALSE }
    }

    const fn ty(&self) -> u64 {
        (self.0 >> 32) & TY_MASK
    }

    const fn id(&self) -> u64 {
        self.0 & ID_MASK
    }

    fn is_predefined(&self) -> bool {
        self.id() < ID_MAX
    }

    pub fn is_number(&self) -> bool {
        self.0 == NAN.0 || self.0 & QUIET_NAN != QUIET_NAN
    }

    pub fn is_object(&self) -> bool {
        self.ty() == TY_OBJECT
    }

    pub fn is_function(&self) -> bool {
        self.ty() == TY_FUNCTION
    }

    pub fn is_string(&self) -> bool {
        self.ty() == TY_STRING
    }

    pub fn is_boolean(&self) -> bool {
        self.eq(&TRUE) || self.eq(&FALSE)
    }

    fn is_ref(&self) -> bool {
        !(self.is_predefined() || self.is_number())
    }

    pub fn get(&self, p: &str) -> Value {
        debug_assert!(self.is_object());
        let mut ret = UNDEFINED;
        unsafe { sys::get(self.0, p.as_ptr(), p.len() as u32, &mut ret.0) };
        ret
    }

    pub fn set(&self, p: &str, value: &Self) {
        debug_assert!(self.is_object());
        unsafe { sys::set(self.0, p.as_ptr(), p.len() as u32, &value.0) }
    }

    pub fn call(&self, args: &[Self]) -> Result<Value, Error> {
        debug_assert!(self.is_function());
        let mut ret = UNDEFINED;
        let ok = unsafe { sys::call(self.0, args.as_ptr().cast(), args.len() as u32, &mut ret.0) };
        if ok {
            Ok(ret)
        } else {
            Err(Error { value: ret })
        }
    }

    pub fn construct(&self, args: &[Self]) -> Result<Value, Error> {
        debug_assert!(self.is_function());
        let mut ret = UNDEFINED;
        let ok =
            unsafe { sys::construct(self.0, args.as_ptr().cast(), args.len() as u32, &mut ret.0) };
        if ok {
            Ok(ret)
        } else {
            Err(Error { value: ret })
        }
    }

    pub fn try_as_f64(&self) -> Option<f64> {
        if self.0 == NAN.0 {
            // NOTE: we don't really want to return nan bits that contain id.
            Some(f64::NAN)
        } else if self.0 & QUIET_NAN != QUIET_NAN {
            Some(f64::from_bits(self.0))
        } else {
            None
        }
    }

    pub fn as_f64(&self) -> f64 {
        self.try_as_f64().expect("not a number")
    }

    pub fn try_as_string(&self) -> Option<String> {
        if self.is_string() {
            let mut ptr: u32 = u32::MAX;
            let mut len: u32 = u32::MAX;
            unsafe { sys::string_get(self.0, &mut ptr, &mut len) };
            debug_assert!(ptr != u32::MAX && len != u32::MAX);
            let buf = unsafe { Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize) };
            String::from_utf8(buf).ok()
        } else {
            None
        }
    }

    pub fn as_string(&self) -> String {
        self.try_as_string().expect("not a string")
    }

    pub fn try_as_bool(&self) -> Option<bool> {
        match () {
            _ if self.0 == TRUE.0 => Some(true),
            _ if self.0 == FALSE.0 => Some(false),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> bool {
        self.try_as_bool().expect("not a bool")
    }
}

// ----
// error

#[derive(Clone)]
pub struct Error {
    value: Value,
}

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

// NOTE: derive(Debug) prints the underlying Value.
//   it's not great when .expect is used.
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self as &dyn fmt::Display).fmt(f)
    }
}

// ----
// closure

pub struct Closure<F: ?Sized> {
    _f: Box<F>,
    value: Value,
}

impl Closure<dyn FnMut()> {
    pub fn new<F>(mut f: Box<F>) -> Self
    where
        F: FnMut() + 'static,
    {
        #[inline(never)]
        extern "C" fn call_by_ptr<F>(ptr: *mut ())
        where
            F: FnMut() + 'static,
        {
            debug_assert!(!ptr.is_null());
            let f: &mut F = unsafe { &mut *(ptr as *mut F) };
            f();
        }

        let mut value = UNDEFINED;
        unsafe { sys::closure_new(call_by_ptr::<F>, &raw mut *f as *mut (), &mut value.0) };

        Self { _f: f, value }
    }
}
