use std::fs::File;
use std::io::{self, Read as _, Seek as _, SeekFrom};
use std::{env, mem, slice};

use mars::alloc::{self, Allocator, ArenaAllocator, ErasedAllocator};
use mars::boxed::Box;
use mars::dropguard::DropGuard;
use stb_sys::*;

fn read_entire_file_or_die<A: Allocator>(filepath: &str, alloc: A) -> Box<[u8], A> {
    let mut f = File::open(filepath).expect("could not open");
    let len = f.seek(SeekFrom::End(0)).expect("could not seek to end to determine len");
    let _ = f.seek(SeekFrom::Start(0)).expect("could seek back to start");
    let mut buf = unsafe { Box::<[u8], _>::new_uninit_in(len as usize, alloc).assume_init() };
    f.read_exact(buf.as_mut()).expect("could not read");
    buf
}

fn main() {
    let arena = ArenaAllocator::new_in(alloc::Global, None);

    let maybe_font_path = env::args().skip(1).next();
    let font_path = match &maybe_font_path {
        Some(font_path) => font_path.as_str(),
        None => panic!("i've got no default fonts"),
    };

    let font_data = read_entire_file_or_die(font_path, &arena);

    unsafe {
        let userdata = &ErasedAllocator::new(&arena) as *const _ as _;
        let mut info = stbtt_fontinfo { userdata, ..mem::zeroed() };
        let ok = stbtt_InitFont(
            &mut info,
            font_data.as_ptr(),
            stbtt_GetFontOffsetForIndex(font_data.as_ptr(), 0),
        );
        if ok == 0 {
            panic!("could not init font");
        }

        // sdf
        // cargo run --example=truetype -- ~/Downloads/unifont-17.0.04.otf | feh -
        {
            let _arena_checkpoint = arena.checkpoint();

            let scale = stbtt_ScaleForPixelHeight(&info, 64.0);
            let (mut width, mut height, mut xoff, mut yoff) = (0, 0, 0, 0);
            let sdf = stbtt_GetCodepointSDF(
                &info,       // info: *const stbtt_fontinfo,
                scale,       // scale: f32,
                'h' as _,    // codepoint: ::core::ffi::c_int,
                8,           // padding: ::core::ffi::c_int,
                128,         // onedge_value: ::core::ffi::c_uchar,
                16.0,        // pixel_dist_scale: f32,
                &mut width,  // width: *mut ::core::ffi::c_int,
                &mut height, // height: *mut ::core::ffi::c_int,
                &mut xoff,   // xoff: *mut ::core::ffi::c_int,
                &mut yoff,   // yoff: *mut ::core::ffi::c_int,
            );
            assert!(arena.is_this_your_memory(sdf));
            let _sdf_guard = DropGuard::new(|| stbtt_FreeSDF(sdf, userdata));
            let sdf = slice::from_raw_parts(sdf, width as usize * height as usize);
            use io::Write as _;
            let mut w = io::stdout();
            write!(
                &mut w,
                "P7\nWIDTH {width}\nHEIGHT {height}\nDEPTH 1\nMAXVAL 255\nTUPLTYPE GRAYSCALE\nENDHDR\n"
            )
            .unwrap();
            w.write_all(sdf).unwrap();
        }
    }
}
