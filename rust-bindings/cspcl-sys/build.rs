use std::env;
use std::path::{Path, PathBuf};

/// C sources whose changes should trigger a rebuild.
const SOURCE_FILES: [&str; 3] = ["cspcl.c", "cspcl.h", "cspcl_config.h"];

/// Environment variables used to locate libcsp.
const BUILD_ENV_VARS: [&str; 3] = ["CSP_INCLUDE_DIR", "CSP_REPO_DIR", "CSP_BUILD_DIR"];

/// Resolved location of the libcsp headers and compiled library.
struct Libcsp {
    include_dirs: Vec<PathBuf>,
    lib_dir: PathBuf,
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");

    let source_dir = resolve_source_dir(&manifest_dir, workspace_root);
    let libcsp = resolve_libcsp();

    for file in SOURCE_FILES {
        println!("cargo:rerun-if-changed={}", source_dir.join(file).display());
    }
    for var in BUILD_ENV_VARS {
        println!("cargo:rerun-if-env-changed={var}");
    }

    let header = source_dir.join("cspcl.h");
    assert!(header.exists(), "cspcl.h not found at {}", header.display());

    compile_native(&source_dir, &libcsp);
    generate_bindings(&header, &source_dir, &libcsp);
}

/// Locate the cspcl C sources (vendored `c_src/` or the workspace `src/`).
fn resolve_source_dir(manifest_dir: &Path, workspace_root: &Path) -> PathBuf {
    let candidates = [manifest_dir.join("c_src"), workspace_root.join("src")];
    candidates
        .iter()
        .find(|dir| dir.exists())
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "cspcl C sources not found. Looked in:\n  {}\n  {}",
                candidates[0].display(),
                candidates[1].display()
            )
        })
}

/// Locate libcsp via `CSP_INCLUDE_DIR`, or a checkout pointed to by `CSP_REPO_DIR`.
fn resolve_libcsp() -> Libcsp {
    let mut include_dirs = Vec::new();
    let mut lib_dir = None;

    if let Ok(dir) = env::var("CSP_INCLUDE_DIR") {
        include_dirs.push(PathBuf::from(dir));
    } else if let Ok(repo) = env::var("CSP_REPO_DIR") {
        let repo = PathBuf::from(repo);
        include_dirs.push(repo.join("include"));

        let build_dir = env::var("CSP_BUILD_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| repo.join("build"));
        if build_dir.join("include").exists() {
            include_dirs.push(build_dir.join("include"));
        }
        if build_dir.join("libcsp.a").exists() || build_dir.join("libcsp.so").exists() {
            lib_dir = Some(build_dir);
        }
    }

    include_dirs.retain(|dir| dir.exists());

    match (include_dirs.is_empty(), lib_dir.filter(|dir| dir.exists())) {
        (false, Some(lib_dir)) => Libcsp {
            include_dirs,
            lib_dir,
        },
        _ => panic!(
            "libcsp not found. Set CSP_INCLUDE_DIR to the headers, or CSP_REPO_DIR to a \
             libcsp checkout with include/ and a build/ containing libcsp.a or libcsp.so."
        ),
    }
}

fn compile_native(source_dir: &Path, libcsp: &Libcsp) {
    let mut build = cc::Build::new();
    build.file(source_dir.join("cspcl.c")).include(source_dir);
    for dir in &libcsp.include_dirs {
        build.include(dir);
    }

    println!(
        "cargo:rustc-link-search=native={}",
        libcsp.lib_dir.display()
    );
    println!("cargo:rustc-link-lib=static=csp");
    println!("cargo:rustc-link-lib=zmq");
    println!("cargo:rustc-link-lib=socketcan");

    if cfg!(target_os = "linux") {
        build.define("__linux__", None);
        println!("cargo:rustc-link-lib=rt");
    }
    println!("cargo:rustc-link-lib=bz2");

    build.compile("cspcl_native");
}

fn generate_bindings(header: &Path, source_dir: &Path, libcsp: &Libcsp) {
    let mut builder = bindgen::Builder::default()
        .header(header.to_string_lossy())
        .clang_arg(format!("-I{}", source_dir.display()))
        .allowlist_type("cspcl_.*")
        .allowlist_type("csp_iface_type")
        .allowlist_function("cspcl_.*")
        .allowlist_var("CSPCL_.*")
        .allowlist_var("csp_iface_type_.*")
        .generate_comments(true)
        .derive_debug(true)
        .derive_default(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    for dir in &libcsp.include_dirs {
        builder = builder.clang_arg(format!("-I{}", dir.display()));
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    builder
        .generate()
        .expect("unable to generate bindings")
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("couldn't write bindings");
}

