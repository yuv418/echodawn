fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    prost_build::compile_protos(&["proto/edcs_proto.proto"], &["proto/"])
        .expect("Failed to compile protobufs");
}
