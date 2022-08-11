use std::path::PathBuf;

extern crate bindgen;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let libedss_path = manifest_dir.parent().unwrap().join("EDSS").join("build");

    println!("cargo:rustc-link-search={}", libedss_path.to_str().unwrap());
    println!("cargo:rustc-link-lib=edss");

    let bindings = bindgen::Builder::default()
        .header("bindings/edssWrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Failed to generate edssInterface bindings.");

    bindings
        .write_to_file("bindings/edss_bindings.rs")
        .expect("Failed to write edssInterface bindings.");

    prost_build::compile_protos(&["proto/edcs_proto.proto"], &["proto/"])
        .expect("Failed to compile protobufs");
}
