use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let proto_path = manifest_dir
        .parent()
        .unwrap()
        .join("EDCSProtocol")
        .join("proto");

    prost_build::compile_protos(&[proto_path.join("edcs_proto.proto")], &[proto_path])
        .expect("Failed to compile protobufs");

    cxx_build::bridge("src/edc_decoder/mod.rs")
        .file("src/edc_decoder/cpp_decoder/src/decoder.cc")
        .compile("cxxbridge-decoder");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/edc_decoder/decoder.cc");
    println!("cargo:rerun-if-changed=src/edc_decoder/decoder.h");
    println!("cargo:rerun-if-changed=src/edc_decoder/mod.rs");
    println!("cargo:rustc-link-search=/usr/lib");
    println!("cargo:rustc-link-lib=avutil");
    println!("cargo:rustc-link-lib=avcodec");
    println!("cargo:rustc-link-lib=avformat");
    println!("cargo:rustc-link-lib=swscale");
}
