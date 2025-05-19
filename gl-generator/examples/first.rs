use std::fs;
use std::io::stdout;

use gl_generator;

// example driven development

fn main() -> anyhow::Result<()> {
    let input = fs::read_to_string("gl-specs/gl.xml")?;
    let registry = gl_generator::filter_registry(
        gl_generator::parse_registry(input.as_str())?,
        "gl",
        (4, 6),
        &[],
    )?;
    let mut w = stdout();
    gl_generator::emit_types(&mut w)?;
    gl_generator::emit_enums(&mut w, &registry)?;
    gl_generator::emit_api(&mut w, &registry)?;
    Ok(())
}
