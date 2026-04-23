use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = PathBuf::from(&manifest_dir);

    // Canonical C sources for cspcl-sys are vendored in c_src.
    let src_in_crate = manifest_path.join("c_src");

    let src_dir = if src_in_crate.exists() {
        src_in_crate
    } else {
        panic!(
            "C source directory not found. Expected:\n  {}",
            src_in_crate.display()
        );
    };

    println!("Using C sources from: {}", src_dir.display());

    // Tell cargo to look for shared libraries in the specified directory
    println!("cargo:rustc-link-search={}", src_dir.display());
    println!("cargo:rustc-link-lib=bz2");

    let header_path = src_dir.join("cspcl.h");

    if !header_path.exists() {
        panic!("cspcl.h not found at: {}", header_path.display());
    }

    println!("cargo:rerun-if-env-changed=CSP_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=CSP_REPO_DIR");

    // Resolve libcsp include directories.
    // CSP_INCLUDE_DIR: explicit path to libcsp's include/ directory.
    // CSP_REPO_DIR: path to the libcsp repo root; we add both include/ and
    //               build/include/ (where csp_autoconfig.h is generated).
    let mut csp_include_dirs: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = env::var("CSP_INCLUDE_DIR") {
        csp_include_dirs.push(PathBuf::from(dir));
    } else if let Ok(repo) = env::var("CSP_REPO_DIR") {
        let repo_path = PathBuf::from(&repo);
        csp_include_dirs.push(repo_path.join("include"));
        // csp_autoconfig.h is generated into build/include by cmake
        let build_include = repo_path.join("build").join("include");
        if build_include.exists() {
            csp_include_dirs.push(build_include);
        }
    } else {
        panic!(
            "libcsp headers not found. Set CSP_INCLUDE_DIR (path to libcsp include/) \
             or CSP_REPO_DIR (path to libcsp repo root)."
        );
    }

    let mut builder = bindgen::Builder::default()
        .header(header_path.to_string_lossy().to_string())
        .clang_arg(format!("-I{}", src_dir.display()));
    for inc in &csp_include_dirs {
        builder = builder.clang_arg(format!("-I{}", inc.display()));
    }
    let bindings = builder
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
