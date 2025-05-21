use std::io::stdout;
use std::{env, fs};

use anyhow::bail;
use gl_generator;

// example driven development

fn main() -> anyhow::Result<()> {
    let command = env::args().nth(1);

    let args: &[(String, gl_generator::Api, gl_generator::Version)] =
        match command.as_ref().map(|string| string.as_str()) {
            Some("egl") => &[(
                fs::read_to_string("gl-specs/egl.xml")?,
                gl_generator::Api::Egl,
                gl_generator::Version(1, 5),
            )],
            Some("gl") => &[(
                fs::read_to_string("gl-specs/gl.xml")?,
                gl_generator::Api::Gl,
                gl_generator::Version(4, 5),
            )],
            Some(_) => bail!("invalid command"),
            None => &[
                (
                    fs::read_to_string("gl-specs/egl.xml")?,
                    gl_generator::Api::Egl,
                    gl_generator::Version(1, 5),
                ),
                (
                    fs::read_to_string("gl-specs/gl.xml")?,
                    gl_generator::Api::Gl,
                    gl_generator::Version(4, 5),
                ),
            ],
        };

    for (spec, api, version) in args {
        let registry = gl_generator::filter_registry(
            gl_generator::parse_registry(&spec)?,
            &api,
            &version,
            &[],
        )?;
        let mut w = stdout();
        gl_generator::emit_types(&mut w, &api)?;
        gl_generator::emit_enums(&mut w, &registry, &api)?;
        gl_generator::emit_api(&mut w, &registry, &api)?;
    }

    Ok(())
}
