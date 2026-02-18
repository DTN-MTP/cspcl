use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let src_dir = PathBuf::from(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("src");

    // Tell cargo to look for shared libraries in the specified directory
    println!("cargo:rustc-link-search={}", src_dir.display());

    // Tell cargo to tell rustc to link the system bzip2
    // shared library.
    println!("cargo:rustc-link-lib=bz2");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let header_path = src_dir.join("cspcl.h");
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header(header_path.to_string_lossy().to_string())
        // Include directory for dependencies
        .clang_arg(format!("-I{}", src_dir.display()))
        // Allowlist for exported types and functions
        .allowlist_type("cspcl_.*")
        .allowlist_function("cspcl_.*")
        .allowlist_var("CSPCL_.*")
        // Generate documentation comments
        .generate_comments(true)
        // Derive Debug, Clone, Copy for types
        .derive_debug(true)
        .derive_default(true)
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
