use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::{env, fs};

fn generate_gl(
    api: gl_generator::Api,
    version: gl_generator::Version,
    extensions: &[&str],
) -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=../gl-generator");
    println!("cargo:rerun-if-changed=../gl-specs");

    let api_str = api.as_str();
    let out_dir = PathBuf::from(&env::var("OUT_DIR")?);

    let spec = fs::read_to_string(format!("../gl-specs/{api_str}.xml"))?;
    let registry = gl_generator::filter_registry(
        gl_generator::parse_registry(spec.as_str())?,
        &api,
        &version,
        extensions,
    )?;

    let mut types_out = BufWriter::new(File::create(
        out_dir.join(format!("{api_str}_types_generated.rs")),
    )?);
    gl_generator::emit_types(&mut types_out, &api)?;

    let mut enums_out = BufWriter::new(File::create(
        out_dir.join(format!("{api_str}_enums_generated.rs")),
    )?);
    gl_generator::emit_enums(&mut enums_out, &registry, &api)?;

    let wasm = env::var("CARGO_CFG_TARGET_FAMILY").is_ok_and(|var| var.as_str() == "wasm");
    if !wasm {
        let mut api_out = BufWriter::new(File::create(
            out_dir.join(format!("{api_str}_api_generated.rs")),
        )?);
        gl_generator::emit_api(&mut api_out, &registry, &api)?;
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");

    generate_gl(gl_generator::Api::Gl, gl_generator::Version(4, 6), &[])?;
    generate_gl(
        gl_generator::Api::Egl,
        gl_generator::Version(1, 5),
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
    )?;

    Ok(())
}
