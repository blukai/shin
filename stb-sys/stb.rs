pub use bindings::*;

#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

mod imp {
    use core::alloc::Layout;
    use core::ffi::c_void;
    use std::ptr::NonNull;

    use mars::alloc::{Allocator, ErasedAllocator, Global};

    // QUOTE: >
    //    - There are a lot of memory allocations. We should modify it to take
    //      a temp buffer and allocate from the temp buffer (without freeing),
    //      should help performance a lot.

    #[repr(C, align(8))]
    struct AllocationHeader {
        // NOTE: stb won't need usize-much memory.
        size: u32,
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn rust_stbtt_malloc(size: usize, userdata: *mut c_void) -> *mut c_void {
        unsafe {
            let size_including_header = size + size_of::<AllocationHeader>();
            let alignment = align_of::<AllocationHeader>();
            let layout =
                Layout::from_size_align(size_including_header, alignment).expect("invalid layout");

            let ptr = if userdata.is_null() {
                Allocator::allocate(&Global, layout)
            } else {
                (*(userdata as *mut ErasedAllocator)).allocate(layout)
            }
            .expect("oom")
            .as_ptr() as *mut u8;

            let header = &mut *(ptr as *mut AllocationHeader);
            header.size = size as u32;
            ptr.add(size_of::<AllocationHeader>()) as *mut c_void
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn rust_stbtt_free(ptr: *mut c_void, userdata: *mut c_void) {
        unsafe {
            // NOTE: idk if this can ever be true, but better be safe or something.
            let Some(nonnull) = NonNull::new(ptr as *mut u8) else {
                return;
            };

            let ptr = ptr.sub(size_of::<AllocationHeader>());
            let header = &*(ptr as *mut AllocationHeader);
            let size_including_header = header.size as usize + size_of::<AllocationHeader>();
            let alignment = align_of::<AllocationHeader>();
            let layout =
                Layout::from_size_align(size_including_header, alignment).expect("invalid layout");

            if userdata.is_null() {
                Allocator::deallocate(&Global, nonnull, layout)
            } else {
                (*(userdata as *mut ErasedAllocator)).deallocate(nonnull, layout)
            }
        }
    }
}
