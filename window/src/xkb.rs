use std::{ffi::c_int, ptr::null_mut};

use crate::libxkbcommon::*;

use anyhow::anyhow;

struct Context {
    context: *mut xkb_context,
    keymap: *mut xkb_keymap,
    state: *mut xkb_state,

    lib: Lib,
}

impl Context {
    unsafe fn from_fd(fd: c_int, size: u32) -> anyhow::Result<Self> {
        let lib = Lib::load()?;

        let context = unsafe { (lib.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_FLAGS) };
        if context.is_null() {
            return Err(anyhow!("could not create xkb context"));
        }

        let keymap_string = unsafe {
            libc::mmap(
                null_mut(),
                size as _,
                libc::PROT_READ,
                libc::MAP_PRIVATE,
                fd,
                0,
            )
        };
        let keymap = unsafe {
            (lib.xkb_keymap_new_from_string)(
                context,
                keymap_string as _,
                xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            )
        };
        if keymap.is_null() {
            unsafe { libc::munmap(keymap_string, size as _) };
            return Err(anyhow!("could not create keymap from string"));
        }

        let state = unsafe { (lib.xkb_state_new)(keymap) };
        if state.is_null() {
            unsafe { libc::munmap(keymap_string, size as _) };
            return Err(anyhow!("could not create state"));
        }

        unsafe { libc::munmap(keymap_string, size as _) };

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
