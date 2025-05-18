#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
pub mod sys {
    include!(concat!(env!("OUT_DIR"), "/gl_generated.rs"));
}
