use std::{
    ffi::{c_char, c_int},
    ptr::null_mut,
};

use crate::libxkbcommon::*;

use anyhow::anyhow;

pub struct Context {
    pub context: *mut xkb_context,
    pub keymap: *mut xkb_keymap,
    pub state: *mut xkb_state,

    pub lib: Lib,
}

impl Context {
    pub unsafe fn from_fd(fd: c_int, size: u32) -> anyhow::Result<Self> {
        let lib = Lib::load()?;

        let context = unsafe { (lib.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_FLAGS) };
        if context.is_null() {
            return Err(anyhow!("could not create xkb context"));
        }

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

        let keymap = unsafe {
            (lib.xkb_keymap_new_from_string)(
                context,
                keymap_addr as *const c_char,
                xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            )
        };
        if keymap.is_null() {
            unsafe { libc::munmap(keymap_addr, size as libc::size_t) };
            return Err(anyhow!("could not create keymap from string"));
        }

        let state = unsafe { (lib.xkb_state_new)(keymap) };
        if state.is_null() {
            unsafe { libc::munmap(keymap_addr, size as libc::size_t) };
            return Err(anyhow!("could not create state"));
        }

        unsafe { libc::munmap(keymap_addr, size as libc::size_t) };
        Ok(Self {
            context,
            keymap,
            state,

            lib,
        })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            (self.lib.xkb_state_unref)(self.state);
            (self.lib.xkb_keymap_unref)(self.keymap);
            (self.lib.xkb_context_unref)(self.context);
        }
    }
}
