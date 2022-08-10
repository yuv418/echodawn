extern crate bindgen;

fn main() {
    let bindings = bindgen::Builder::default()
        .header("bindings/edssWrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Failed to generate edssInterface bindings.");

    bindings
        .write_to_file("bindings/edss_bindings.rs")
        .expect("Failed to write edssInterface bindings.");

    capnpc::CompilerCommand::new()
        .file("proto/edcs_proto.capnp")
        .default_parent_module(vec!["edcs_server".to_owned()])
        .run()
        .expect("Failed to compile EDCS protocol!")
}
