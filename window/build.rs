use std::env;
use std::fs::{self, File};
use std::path::PathBuf;

fn generate_wayland_bindings() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=../wayland-scanner");
    println!("cargo:rerun-if-changed=../wayland-protocols");

    let out_dir = PathBuf::from(&env::var("OUT_DIR")?);
    let mut out_file = File::create(out_dir.join("wayland_generated.rs"))?;

    for dir_entry in fs::read_dir("../wayland-protocols")? {
        let file = std::fs::File::open(dir_entry?.path())?;
        let protocol = wayland_scanner::parse_protocol(std::io::BufReader::new(file))?;
        wayland_scanner::generate_protocol(&mut out_file, &protocol)?;
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");

    generate_wayland_bindings()?;

    Ok(())
}