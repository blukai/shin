use std::any::type_name;
use std::ffi::{c_char, c_int};
use std::ptr::null_mut;
use std::{error, fmt, mem};

use crate::libxkbcommon as xkbcommon;

// ----
// keymap+state

#[derive(Debug)]
pub enum KeymapStateCreationError {
    MmapFailed(i32),
    NoKeymap,
    NoState,
}

impl error::Error for KeymapStateCreationError {}

impl fmt::Display for KeymapStateCreationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MmapFailed(errno) => f.write_fmt(format_args!("could not mmap: {errno:#x}")),
            Self::NoKeymap => f.write_str("could not create keymap"),
            Self::NoState => f.write_str("could not create state"),
        }
    }
}

// NOTE: fd and size are stored to ensure that you are not using dangling handle.
#[derive(Clone, Copy, PartialEq, Eq)]
struct KeymapStateId {
    fd: c_int,
    size: u32,
}

#[derive(Clone, Copy)]
pub struct KeymapStateHandle {
    id: KeymapStateId,
}

pub struct KeymapState {
    pub keymap: *mut xkbcommon::xkb_keymap,
    pub state: *mut xkbcommon::xkb_state,
    id: KeymapStateId,
}

impl Drop for KeymapState {
    fn drop(&mut self) {
        panic!(
            "{} must be destroyed by {}",
            type_name::<Self>(),
            type_name::<ApiContext>()
        );
    }
}

// ----
// api+context

#[derive(Debug)]
pub enum ApiContextCreationError {
    Dynlib(dynlib::Error),
    NoContext,
}

impl error::Error for ApiContextCreationError {}

impl fmt::Display for ApiContextCreationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dynlib(err) => err.fmt(f),
            Self::NoContext => f.write_str("could not create context"),
        }
    }
}

pub struct ApiContext {
    pub api: xkbcommon::Api,
    pub context: *mut xkbcommon::xkb_context,
    keymap_state: Option<KeymapState>,
}

impl Drop for ApiContext {
    fn drop(&mut self) {
        if let Some(ks) = self.keymap_state.take() {
            unsafe { (self.api.xkb_state_unref)(ks.state) };
            unsafe { (self.api.xkb_keymap_unref)(ks.keymap) };
            // NOTE: forget to not invoke panicking drop.
            mem::forget(ks);
        }

        unsafe { (self.api.xkb_context_unref)(self.context) };
    }
}

impl ApiContext {
    pub fn new() -> Result<Self, ApiContextCreationError> {
        let api = xkbcommon::Api::load().map_err(ApiContextCreationError::Dynlib)?;

        let context =
            unsafe { (api.xkb_context_new)(xkbcommon::xkb_context_flags::XKB_CONTEXT_NO_FLAGS) };
        if context.is_null() {
            return Err(ApiContextCreationError::NoContext);
        }

        Ok(Self {
            api,
            context,
            keymap_state: None,
        })
    }

    pub fn create_keymap_state_from_fd(
        &mut self,
        fd: c_int,
        size: u32,
    ) -> Result<KeymapStateHandle, KeymapStateCreationError> {
        // you don't need more then one at a time, do you?
        // if yes - change keymap_state to be an array of few.
        assert!(self.keymap_state.is_none());

        let keymap_addr = unsafe {
            libc::mmap(
                null_mut(),
                size as libc::size_t,
                libc::PROT_READ,
                // > From version 7 onwards, the fd must be mapped with MAP_PRIVATE by the
                // recipient, as MAP_SHARED may fail.
                // - https://wayland.app/protocols/wayland#wl_keyboard:event:keymap
                libc::MAP_PRIVATE,
                fd,
                0,
            )
        };
        if keymap_addr == libc::MAP_FAILED {
            let errno = unsafe { *libc::__errno_location() };
            return Err(KeymapStateCreationError::MmapFailed(errno));
        }

        let keymap = unsafe {
            (self.api.xkb_keymap_new_from_string)(
                self.context,
                keymap_addr as *const c_char,
                xkbcommon::xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                xkbcommon::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            )
        };
        if keymap.is_null() {
            unsafe { libc::munmap(keymap_addr, size as libc::size_t) };
            return Err(KeymapStateCreationError::NoKeymap);
        }

        let state = unsafe { (self.api.xkb_state_new)(keymap) };
        if state.is_null() {
            unsafe { (self.api.xkb_keymap_unref)(keymap) };
            unsafe { libc::munmap(keymap_addr, size as libc::size_t) };
            return Err(KeymapStateCreationError::NoState);
        }

        unsafe { libc::munmap(keymap_addr, size as libc::size_t) };

        let id = KeymapStateId { fd, size };
        let handle = KeymapStateHandle { id };

        self.keymap_state = Some(KeymapState { keymap, state, id });

        Ok(handle)
    }

    /// panics if handle is invalid.
    pub fn get_keymap_state(&self, handle: KeymapStateHandle) -> &KeymapState {
        match self.keymap_state.as_ref() {
            Some(ks) if ks.id == handle.id => ks,
            _ => panic!("invalid keymap state handle"),
        }
    }

    /// panics if handle is invalid.
    pub fn remove_keymap_state(&mut self, handle: KeymapStateHandle) {
        let ks = self
            .keymap_state
            .take_if(|ks| ks.id == handle.id)
            .expect("invalid keymap state handle");
        unsafe { (self.api.xkb_state_unref)(ks.state) };
        unsafe { (self.api.xkb_keymap_unref)(ks.keymap) };
        // NOTE: forget to not invoke panicking drop.
        mem::forget(ks);
    }
}
