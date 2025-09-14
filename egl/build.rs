use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    println!("cargo:rerun-if-changed=../khronos-generator");
    println!("cargo:rerun-if-changed=../khronos-registry");

    let out_dir = PathBuf::from(&env::var("OUT_DIR").expect("out dir is missing"));

    let api = khronos_generator::Api::Egl;
    let api_str = api.as_str();

    let spec = fs::read_to_string(format!("../khronos-registry/{api_str}.xml"))
        .expect("could not read specs");
    let registry = khronos_generator::filter_registry(
        khronos_generator::parse_registry(spec.as_str()).expect("could not parse registry"),
        &api,
        &khronos_generator::Version(1, 5),
        &[
            #[cfg(feature = "EGL_EXT_platform_base")]
            "EGL_EXT_platform_base",
            #[cfg(feature = "EGL_EXT_platform_wayland")]
            "EGL_EXT_platform_wayland",
            #[cfg(feature = "EGL_KHR_image")]
            "EGL_KHR_image",
            #[cfg(feature = "EGL_KHR_platform_wayland")]
            "EGL_KHR_platform_wayland",
            #[cfg(feature = "EGL_MESA_image_dma_buf_export")]
            "EGL_MESA_image_dma_buf_export",
        ],
    )
    .expect("could not filter registry");

    let mut types_out = BufWriter::new(
        File::create(out_dir.join(format!("{api_str}_types_generated.rs")))
            .expect("could not create types out file"),
    );
    khronos_generator::emit_types(&mut types_out, &api).expect("could not emit types");

    let mut enums_out = BufWriter::new(
        File::create(out_dir.join(format!("{api_str}_enums_generated.rs")))
            .expect("could not create enums out file"),
    );
    khronos_generator::emit_enums(&mut enums_out, &registry, &api).expect("could not emit enums");

    let mut api_out = BufWriter::new(
        File::create(out_dir.join(format!("{api_str}_api_generated.rs")))
            .expect("could not create api out file"),
    );
    khronos_generator::emit_api(&mut api_out, &registry, &api).expect("could not emit api");
}
