use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::{env, fs};

use gl_generator::gl;

fn generate_gl_native() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=../gl-generator");
    println!("cargo:rerun-if-changed=../gl-specs");

    let out_dir = PathBuf::from(&env::var("OUT_DIR")?);
    let mut out_file = BufWriter::new(File::create(out_dir.join("gl_native.rs"))?);

    let spec = fs::read_to_string("../gl-specs/gl.xml")?;
    let registry = gl::filter_registry(gl::parse_registry(spec.as_str())?, "gl", (4, 6), &[])?;
    gl::generate_api(&mut out_file, &registry)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");

    generate_gl_native()?;

    Ok(())
}
