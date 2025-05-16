use std::io::{BufWriter, stdout};

use gl_generator::gl::{filter_registry, parse_registry};

// example driven development  xd

fn main() -> anyhow::Result<()> {
    let file = std::fs::File::open("gl-specs/gl.xml")?;
    let registry = parse_registry(std::io::BufReader::new(file))?;
    let registry = filter_registry(registry, "gl", (4, 6), &[])?;

    let mut w = BufWriter::new(stdout());
    gl_generator::gl::generate_api(&mut w, &registry)?;

    Ok(())
}
