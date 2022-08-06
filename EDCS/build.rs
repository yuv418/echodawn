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
}
