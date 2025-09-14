use std::io::stdout;
use std::{env, fs};

use anyhow::bail;
use khronos_generator;

// example driven development

fn main() -> anyhow::Result<()> {
    let command = env::args().nth(1);

    let args: &[(String, khronos_generator::Api, khronos_generator::Version)] =
        match command.as_ref().map(|string| string.as_str()) {
            Some("egl") => &[(
                fs::read_to_string("khronos-registry/egl.xml")?,
                khronos_generator::Api::Egl,
                khronos_generator::Version(1, 5),
            )],
            Some("gl") => &[(
                fs::read_to_string("khronos-registry/gl.xml")?,
                khronos_generator::Api::Gl,
                khronos_generator::Version(4, 5),
            )],
            Some(_) => bail!("invalid command"),
            None => &[
                (
                    fs::read_to_string("khronos-registry/egl.xml")?,
                    khronos_generator::Api::Egl,
                    khronos_generator::Version(1, 5),
                ),
                (
                    fs::read_to_string("khronos-registry/gl.xml")?,
                    khronos_generator::Api::Gl,
                    khronos_generator::Version(4, 5),
                ),
            ],
        };

    for (spec, api, version) in args {
        let registry = khronos_generator::filter_registry(
            khronos_generator::parse_registry(&spec)?,
            &api,
            &version,
            &[],
        )?;
        let mut w = stdout();
        khronos_generator::emit_types(&mut w, &api)?;
        khronos_generator::emit_enums(&mut w, &registry, &api)?;
        khronos_generator::emit_api(&mut w, &registry, &api)?;
    }

    Ok(())
}
