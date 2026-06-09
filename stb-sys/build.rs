use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=../third-party/stb");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=wrapper.c");

    let out_dir = env::var("OUT_DIR").unwrap();

    cc::Build::new().file("wrapper.c").compile("stb");

    let bindings = bindgen::builder()
        .use_core()
        .header("wrapper.c")
        // TODO: can i just do an allowlist_file instead?
        .allowlist_function(r"stbtt_.*")
        .allowlist_type(r"stbtt_.*")
        .layout_tests(false)
        // NOTE: tell cargo to invalidate the built crate whenever any of the included header files
        // changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("could not generate bindings");
    let out_dir = PathBuf::from(&out_dir);
    let bindings_path = out_dir.join("bindings.rs");
    bindings.write_to_file(&bindings_path).expect("failed to write bindings");
}
