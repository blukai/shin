use std::ffi::{c_char, c_int};
use std::ptr::null_mut;

use anyhow::anyhow;
use scopeguard::ScopeGuard;
use xkbcommon::*;

pub struct ApiContextPair {
    pub api: xkbcommon::Api,
    pub context: *mut xkbcommon::xkb_context,
}

impl ApiContextPair {
    pub fn new() -> anyhow::Result<Self> {
        let api = Api::load()?;

        let context = unsafe { (api.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_FLAGS) };
        if context.is_null() {
            return Err(anyhow!("could not create xkb context"));
        }

        Ok(Self { api, context })
    }

    pub fn deinit(self) {
        unsafe { (self.api.xkb_context_unref)(self.context) };
    }
}

pub struct KeymapStatePair {
    pub keymap: *mut xkb_keymap,
    pub state: *mut xkb_state,
}

impl KeymapStatePair {
    pub fn from_fd(fd: c_int, size: u32, acp: &ApiContextPair) -> anyhow::Result<Self> {
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
            return Err(anyhow!("could not mmap fd: 0x:{errno:x}"));
        }
        let _keymap_munmap = ScopeGuard::new(|| {
            unsafe { libc::munmap(keymap_addr, size as libc::size_t) };
        });

        let keymap = unsafe {
            (acp.api.xkb_keymap_new_from_string)(
                acp.context,
                keymap_addr as *const c_char,
                xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            )
        };
        if keymap.is_null() {
            return Err(anyhow!("could not create keymap from string"));
        }

        let state = unsafe { (acp.api.xkb_state_new)(keymap) };
        if state.is_null() {
            return Err(anyhow!("could not create state"));
        }

        Ok(Self { keymap, state })
    }

    pub fn deinit(self, acp: &ApiContextPair) {
        unsafe { (acp.api.xkb_state_unref)(self.state) };
        unsafe { (acp.api.xkb_keymap_unref)(self.keymap) };
    }
}
