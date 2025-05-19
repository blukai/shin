use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::{env, fs};

use gl_generator;

fn generate_gl() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=../gl-generator");
    println!("cargo:rerun-if-changed=../gl-specs");

    let out_dir = PathBuf::from(&env::var("OUT_DIR")?);

    let spec = fs::read_to_string("../gl-specs/gl.xml")?;
    let registry = gl_generator::filter_registry(
        gl_generator::parse_registry(spec.as_str())?,
        "gl",
        (4, 6),
        &[],
    )?;

    let mut types_out = BufWriter::new(File::create(out_dir.join("gl_types.rs"))?);
    gl_generator::emit_types(&mut types_out)?;

    let mut enums_out = BufWriter::new(File::create(out_dir.join("gl_enums.rs"))?);
    gl_generator::emit_enums(&mut enums_out, &registry)?;

    let mut api_out = BufWriter::new(File::create(out_dir.join("gl_api.rs"))?);
    gl_generator::emit_api(&mut api_out, &registry)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");

    // TODO: generate api only on native, but not on wasm
    // env::var("CARGO_CFG_TARGET_ARCH")

    generate_gl()?;

    Ok(())
}
