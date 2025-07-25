use std::env;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if !env::var("CARGO_CFG_UNIX").is_ok() {
        panic!("wayland is a unix-only thing, isn't it?");
    }

    println!("cargo:rerun-if-changed=../wayland-scanner");
    println!("cargo:rerun-if-changed=../wayland-protocols");

    let out_dir =
        PathBuf::from(&env::var("OUT_DIR").expect("OUT_DIR env var is missing or invalid"));
    let out_file =
        File::create(out_dir.join("wayland_generated.rs")).expect("could not create out file");
    let mut w = BufWriter::new(out_file);

    for entry in fs::read_dir("../wayland-protocols").expect("could not read protocols dir") {
        let entry = entry.expect("could not read protocols dir entry");
        let input = fs::read_to_string(entry.path()).expect("could not read protocol");
        let protocol =
            wayland_scanner::parse_protocol(input.as_str()).expect("could not parse protocol");
        wayland_scanner::emit_protocol(&mut w, &protocol).expect("could not generate protocol");
    }
}
