use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug)]
enum CspBuildMode {
    Real {
        include_dirs: Vec<PathBuf>,
        lib_dir: PathBuf,
    },
    Stub {
        include_dir: PathBuf,
        stub_source: PathBuf,
    },
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let source_dir = resolve_cspcl_source_dir(&manifest_dir, workspace_root);
    let build_mode = resolve_csp_build_mode(workspace_root);

    println!(
        "cargo:rerun-if-changed={}",
        source_dir.join("cspcl.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        source_dir.join("cspcl.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        source_dir.join("cspcl_config.h").display()
    );
    println!("cargo:rerun-if-env-changed=CSP_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=CSP_REPO_DIR");
    println!("cargo:rerun-if-env-changed=CSP_BUILD_DIR");
    println!("cargo:rerun-if-env-changed=CSP_USE_STUBS");

    let header_path = source_dir.join("cspcl.h");
    if !header_path.exists() {
        panic!("cspcl.h not found at {}", header_path.display());
    }

    compile_native_objects(&source_dir, &build_mode);
    generate_bindings(&header_path, &source_dir, &build_mode);
}

fn resolve_cspcl_source_dir(manifest_dir: &Path, workspace_root: &Path) -> PathBuf {
    let src_in_crate = manifest_dir.join("c_src");
    if src_in_crate.exists() {
        return src_in_crate;
    }

    let src_in_workspace = workspace_root.join("src");
    if src_in_workspace.exists() {
        return src_in_workspace;
    }

    panic!(
        "C source directory not found. Looked in:\n  {}\n  {}",
        src_in_crate.display(),
        src_in_workspace.display()
    );
}

fn resolve_csp_build_mode(workspace_root: &Path) -> CspBuildMode {
    if env::var("CSP_USE_STUBS").ok().as_deref() == Some("1") {
        return stub_build_mode(workspace_root);
    }

    if let Some(real_mode) = resolve_real_csp_mode() {
        return real_mode;
    }

    stub_build_mode(workspace_root)
}

fn stub_build_mode(workspace_root: &Path) -> CspBuildMode {
    let stub_include_dir = workspace_root.join("stubs");
    let stub_source = workspace_root.join("stubs").join("csp_stub.c");
    if !stub_include_dir.exists() || !stub_source.exists() {
        panic!(
            "CSP stubs not found. Expected {} and {}",
            stub_include_dir.display(),
            stub_source.display()
        );
    }

    println!(
        "cargo:warning=libcsp headers/library not found; building cspcl-sys against bundled CSP stubs"
    );
    println!("cargo:rerun-if-changed={}", stub_source.display());
    println!(
        "cargo:rerun-if-changed={}",
        stub_include_dir.join("csp").join("csp.h").display()
    );

    CspBuildMode::Stub {
        include_dir: stub_include_dir,
        stub_source,
    }
}

fn resolve_real_csp_mode() -> Option<CspBuildMode> {
    let mut include_dirs = Vec::new();
    let mut lib_dir = None;

    if let Ok(dir) = env::var("CSP_INCLUDE_DIR") {
        include_dirs.push(PathBuf::from(dir));
    } else if let Ok(repo) = env::var("CSP_REPO_DIR") {
        let repo_path = PathBuf::from(&repo);
        include_dirs.push(repo_path.join("include"));

        let build_dir = env::var("CSP_BUILD_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| repo_path.join("build"));
        let generated_include = build_dir.join("include");
        if generated_include.exists() {
            include_dirs.push(generated_include);
        }
        if build_dir.join("libcsp.a").exists() || build_dir.join("libcsp.so").exists() {
            lib_dir = Some(build_dir);
        }
    }

    include_dirs.retain(|dir| dir.exists());
    let lib_dir = lib_dir.filter(|dir| dir.exists());

    match (include_dirs.is_empty(), lib_dir) {
        (false, Some(lib_dir)) => Some(CspBuildMode::Real {
            include_dirs,
            lib_dir,
        }),
        _ => None,
    }
}

fn compile_native_objects(source_dir: &Path, build_mode: &CspBuildMode) {
    let mut build = cc::Build::new();
    build.file(source_dir.join("cspcl.c"));
    build.include(source_dir);

    match build_mode {
        CspBuildMode::Real {
            include_dirs,
            lib_dir,
        } => {
            for include_dir in include_dirs {
                build.include(include_dir);
            }
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-lib=static=csp");
            println!("cargo:rustc-link-lib=zmq");
            println!("cargo:rustc-link-lib=socketcan");
        }
        CspBuildMode::Stub {
            include_dir,
            stub_source,
        } => {
            build.include(include_dir);
            build.file(stub_source);
        }
    }

    if cfg!(target_os = "linux") {
        build.define("__linux__", None);
        println!("cargo:rustc-link-lib=rt");
    }

    println!("cargo:rustc-link-lib=bz2");
    build.compile("cspcl_native");
}

fn generate_bindings(header_path: &Path, source_dir: &Path, build_mode: &CspBuildMode) {
    let mut builder = bindgen::Builder::default()
        .header(header_path.to_string_lossy().into_owned())
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

    match build_mode {
        CspBuildMode::Real { include_dirs, .. } => {
            for include_dir in include_dirs {
                builder = builder.clang_arg(format!("-I{}", include_dir.display()));
            }
        }
        CspBuildMode::Stub { include_dir, .. } => {
            builder = builder.clang_arg(format!("-I{}", include_dir.display()));
        }
    }

    let bindings = builder.generate().expect("Unable to generate bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings");
}
