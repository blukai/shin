#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int};
use std::marker;

use dynlib::DynLib;

// *Real* modifiers names are hardcoded in libxkbcommon
pub const XKB_MOD_NAME_SHIFT: &[u8] = b"Shift\0";
pub const XKB_MOD_NAME_CAPS: &[u8] = b"Lock\0";
pub const XKB_MOD_NAME_CTRL: &[u8] = b"Control\0";
pub const XKB_MOD_NAME_MOD1: &[u8] = b"Mod1\0";
pub const XKB_MOD_NAME_MOD2: &[u8] = b"Mod2\0";
pub const XKB_MOD_NAME_MOD3: &[u8] = b"Mod3\0";
pub const XKB_MOD_NAME_MOD4: &[u8] = b"Mod4\0";
pub const XKB_MOD_NAME_MOD5: &[u8] = b"Mod5\0";

// Usual virtual modifiers mappings to real modifiers
pub const XKB_MOD_NAME_ALT: &[u8] = b"Mod1\0"; // Alt
pub const XKB_MOD_NAME_LOGO: &[u8] = b"Mod4\0"; // Super
pub const XKB_MOD_NAME_NUM: &[u8] = b"Mod2\0"; // NumLock

#[repr(C)]
pub struct xkb_context {
    _data: (),
    _marker: marker::PhantomData<(*mut u8, marker::PhantomPinned)>,
}

#[repr(C)]
pub struct xkb_keymap {
    _data: (),
    _marker: marker::PhantomData<(*mut u8, marker::PhantomPinned)>,
}

#[repr(C)]
pub struct xkb_state {
    _data: (),
    _marker: marker::PhantomData<(*mut u8, marker::PhantomPinned)>,
}

pub type xkb_layout_index_t = u32;
pub type xkb_mod_index_t = u32;
pub type xkb_mod_mask_t = u32;
pub type xkb_keycode_t = u32;
pub type xkb_keysym_t = u32;

#[repr(C)]
#[derive(Debug, Clone)]
pub enum xkb_context_flags {
    XKB_CONTEXT_NO_FLAGS = 0,
    XKB_CONTEXT_NO_DEFAULT_INCLUDES = (1 << 0),
    XKB_CONTEXT_NO_ENVIRONMENT_NAMES = (1 << 1),
}

#[repr(C)]
#[derive(Debug, Clone)]
pub enum xkb_keymap_format {
    XKB_KEYMAP_USE_ORIGINAL_FORMAT = 0,
    XKB_KEYMAP_FORMAT_TEXT_V1 = 1,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub enum xkb_keymap_compile_flags {
    XKB_KEYMAP_COMPILE_NO_FLAGS = 0,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub enum xkb_state_component {
    XKB_STATE_MODS_DEPRESSED = (1 << 0),
    XKB_STATE_MODS_LATCHED = (1 << 1),
    XKB_STATE_MODS_LOCKED = (1 << 2),
    XKB_STATE_MODS_EFFECTIVE = (1 << 3),
    XKB_STATE_LAYOUT_DEPRESSED = (1 << 4),
    XKB_STATE_LAYOUT_LATCHED = (1 << 5),
    XKB_STATE_LAYOUT_LOCKED = (1 << 6),
    XKB_STATE_LAYOUT_EFFECTIVE = (1 << 7),
    XKB_STATE_LEDS = (1 << 8),
}

pub struct Lib {
    pub xkb_context_new: unsafe extern "C" fn(flags: xkb_context_flags) -> *mut xkb_context,
    pub xkb_context_unref: unsafe extern "C" fn(context: *mut xkb_context),

    pub xkb_keymap_mod_get_index:
        unsafe extern "C" fn(keymap: *mut xkb_keymap, name: *const c_char) -> xkb_mod_index_t,
    pub xkb_keymap_new_from_string: unsafe extern "C" fn(
        ctx: *mut xkb_context,
        string: *const c_char,
        format: xkb_keymap_format,
        flags: xkb_keymap_compile_flags,
    ) -> *mut xkb_keymap,
    pub xkb_keymap_key_repeats:
        unsafe extern "C" fn(keymap: *mut xkb_keymap, key: xkb_keycode_t) -> c_int,
    pub xkb_keymap_unref: unsafe extern "C" fn(keymap: *mut xkb_keymap),

    pub xkb_state_mod_index_is_active: unsafe extern "C" fn(
        state: *mut xkb_state,
        idx: xkb_mod_index_t,
        r#type: xkb_state_component,
    ) -> c_int,
    pub xkb_state_new: unsafe extern "C" fn(keymap: *mut xkb_keymap) -> *mut xkb_state,
    pub xkb_state_unref: unsafe extern "C" fn(state: *mut xkb_state),
    pub xkb_state_update_mask: unsafe extern "C" fn(
        state: *mut xkb_state,
        base_mods: xkb_mod_mask_t,
        latched_mods: xkb_mod_mask_t,
        locked_mods: xkb_mod_mask_t,
        base_group: xkb_layout_index_t,
        latched_group: xkb_layout_index_t,
        locked_group: xkb_layout_index_t,
    ) -> c_int, // xkb_state_component
    pub xkb_state_key_get_one_sym:
        unsafe extern "C" fn(state: *mut xkb_state, key: xkb_keycode_t) -> xkb_keysym_t,

    pub xkb_keysym_to_utf32: unsafe extern "C" fn(keysym: xkb_keysym_t) -> u32,

    _dynlib: DynLib,
}

impl Lib {
    pub fn load() -> anyhow::Result<Self> {
        let dynlib = DynLib::load(c"libxkbcommon.so")
            .or_else(|_| DynLib::load(c"libxkbcommon.so.0"))
            .or_else(|_| DynLib::load(c"libxkbcommon.so.0.0.0"))?;

        Ok(Self {
            xkb_context_new: dynlib.lookup(c"xkb_context_new")?,
            xkb_context_unref: dynlib.lookup(c"xkb_context_unref")?,

            xkb_keymap_mod_get_index: dynlib.lookup(c"xkb_keymap_mod_get_index")?,
            xkb_keymap_new_from_string: dynlib.lookup(c"xkb_keymap_new_from_string")?,
            xkb_keymap_key_repeats: dynlib.lookup(c"xkb_keymap_key_repeats")?,
            xkb_keymap_unref: dynlib.lookup(c"xkb_keymap_unref")?,

            xkb_state_mod_index_is_active: dynlib.lookup(c"xkb_state_mod_index_is_active")?,
            xkb_state_new: dynlib.lookup(c"xkb_state_new")?,
            xkb_state_unref: dynlib.lookup(c"xkb_state_unref")?,
            xkb_state_update_mask: dynlib.lookup(c"xkb_state_update_mask")?,
            xkb_state_key_get_one_sym: dynlib.lookup(c"xkb_state_key_get_one_sym")?,

            xkb_keysym_to_utf32: dynlib.lookup(c"xkb_keysym_to_utf32")?,

            _dynlib: dynlib,
        })
    }
}
