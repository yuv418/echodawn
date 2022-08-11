use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let proto_path = manifest_dir
        .parent()
        .unwrap()
        .join("EDCSProtocol")
        .join("proto");

    prost_build::compile_protos(&[proto_path.join("edcs_proto.proto")], &[proto_path])
        .expect("Failed to compile protobufs");
}
