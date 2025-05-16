#[derive(Debug, Clone)]
#[repr(transparent)]
struct ExternRef {
    idx: u32,
}

impl ExternRef {
    pub(super) fn is_nil(&self) -> bool {
        self.idx == 0
    }
}

unsafe extern "C" {
    fn gl_clear_color(this: ExternRef, r: f32, g: f32, b: f32, a: f32);
    fn gl_clear(this: ExternRef, mask: u32);
}
