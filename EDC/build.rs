fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    capnpc::CompilerCommand::new()
        .file("proto/edcs_proto.capnp")
        .default_parent_module(vec!["edc_client".to_owned()])
        .run()
        .expect("Failed to compile EDCS protocol!")
}
