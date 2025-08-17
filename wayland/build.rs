use std::env;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::PathBuf;

const PROTOCOLS: &[&str] = &[
    // NOTE: always include core protocols
    "wayland.xml",
    #[cfg(feature = "cursor-shape-v1")]
    "cursor-shape-v1.xml",
    #[cfg(feature = "fractional-scale-v1")]
    "fractional-scale-v1.xml",
    #[cfg(feature = "linux-dmabuf-v1")]
    "linux-dmabuf-v1.xml",
    #[cfg(feature = "pointer-gestures-unstable-v1")]
    "pointer-gestures-unstable-v1.xml",
    #[cfg(feature = "tablet-v2")]
    "tablet-v2.xml",
    #[cfg(feature = "viewporter")]
    "viewporter.xml",
    #[cfg(feature = "wlr-layer-shell-unstable-v1")]
    "wlr-layer-shell-unstable-v1.xml",
    #[cfg(feature = "wlr-screencopy-unstable-v1")]
    "wlr-screencopy-unstable-v1.xml",
    #[cfg(feature = "xdg-shell")]
    "xdg-shell.xml",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // TODO: mac is also unix? this should yell at you on that.
    if !env::var("CARGO_CFG_UNIX").is_ok() {
        panic!("wayland is a unix-only thing, isn't it?");
    }

    println!("cargo:rerun-if-changed=../wayland-scanner");
    println!("cargo:rerun-if-changed=../wayland-protocols");

    let protocols_dir = PathBuf::from("../wayland-protocols");

    let out_dir =
        PathBuf::from(&env::var("OUT_DIR").expect("OUT_DIR env var is missing or invalid"));
    let out_file =
        File::create(out_dir.join("wayland_generated.rs")).expect("could not create out file");
    let mut w = BufWriter::new(out_file);

    for protocol in PROTOCOLS {
        let path = protocols_dir.join(protocol);
        let input = fs::read_to_string(path).expect("could not read protocol");
        let protocol =
            wayland_scanner::parse_protocol(input.as_str()).expect("could not parse protocol");
        wayland_scanner::emit_protocol(&mut w, &protocol).expect("could not generate protocol");
    }
}
