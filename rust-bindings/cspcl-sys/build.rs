use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = PathBuf::from(&manifest_dir);

    // When published to crates.io, the C source files are included in the crate root
    // Check cspcl-sys/src first (where they'll be when published)
    let src_in_crate = manifest_path.join("c_src");
    
    // Otherwise look in workspace (local development)
    let workspace_root = manifest_path.parent().unwrap().parent().unwrap();
    let src_in_workspace = workspace_root.join("src");
    
    let src_dir = if src_in_crate.exists() {
        src_in_crate
    } else if src_in_workspace.exists() {
        src_in_workspace
    } else {
        panic!("C source directory not found. Looked in:\n  {}\n  {}", 
               src_in_crate.display(), 
               src_in_workspace.display());
    };

    println!("Using C sources from: {}", src_dir.display());
    
    // Tell cargo to look for shared libraries in the specified directory
    println!("cargo:rustc-link-search={}", src_dir.display());
    println!("cargo:rustc-link-lib=bz2");

    let header_path = src_dir.join("cspcl.h");
    
    if !header_path.exists() {
        panic!("cspcl.h not found at: {}", header_path.display());
    }
    
    let bindings = bindgen::Builder::default()
        .header(header_path.to_string_lossy().to_string())
        .clang_arg(format!("-I{}", src_dir.display()))
        .allowlist_type("cspcl_.*")
        .allowlist_function("cspcl_.*")
        .allowlist_var("CSPCL_.*")
        .generate_comments(true)
        .derive_debug(true)
        .derive_default(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
