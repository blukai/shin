#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

include!(concat!(env!("OUT_DIR"), "/egl_types_generated.rs"));
include!(concat!(env!("OUT_DIR"), "/egl_enums_generated.rs"));
include!(concat!(env!("OUT_DIR"), "/egl_api_generated.rs"));
