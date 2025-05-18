use std::fs;
use std::io::stdout;

use gl_generator::gl;

// example driven development

fn main() -> anyhow::Result<()> {
    let input = fs::read_to_string("gl-specs/gl.xml")?;
    let registry = gl::parse_registry(input.as_str())?;
    let registry = gl::filter_registry(registry, "gl", (4, 6), &[])?;
    let mut w = stdout();
    gl::generate_api(&mut w, &registry)?;
    Ok(())
}
