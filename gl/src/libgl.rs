mod types {
    include!(concat!(env!("OUT_DIR"), "/gl_types_generated.rs"));
}

#[expect(non_upper_case_globals)]
mod enums {
    use super::types::*;

    include!(concat!(env!("OUT_DIR"), "/gl_enums_generated.rs"));
}

#[expect(non_camel_case_types)]
#[expect(non_snake_case)]
mod api {
    use super::types::*;

    include!(concat!(env!("OUT_DIR"), "/gl_api_generated.rs"));
}

pub use api::Api;
pub use enums::*;
pub use types::*;
