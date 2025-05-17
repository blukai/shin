use std::{
    fs::{self},
    io::{BufWriter, stdout},
};

use gl_generator::gl::{filter_registry, parse_registry};

// example driven development  xd

fn main() -> anyhow::Result<()> {
    let input = fs::read_to_string("gl-specs/gl.xml")?;
    let registry = parse_registry(input.as_str())?;
    let registry = filter_registry(registry, "gl", (4, 6), &[])?;

    let mut w = BufWriter::new(stdout());
    gl_generator::gl::generate_api(&mut w, &registry)?;

    Ok(())
}
