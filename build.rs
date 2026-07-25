
fn main() {     // https://doc.rust-lang.org/stable/cargo/reference/build-scripts.html
    //println!("cargo:rerun-if-changed=build.rs");    // XXX: prevent re-run indead
    // By default, cargo always re-run the build script if any file within the package
    // is changed, and no any rerun-if instruction is emitted.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-env-changed=RIVE_DEFS_DIR");
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}",
        chrono::Local::now().format("%H:%M:%S%z %Y-%m-%d"));

    use std::{env, path::PathBuf};
    let git_hash = std::process::Command::new("git").args(["rev-parse", "--short", "HEAD"])
        .output().ok().filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|hash| hash.trim().to_owned()).filter(|hash| !hash.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=BUILD_GIT_HASH={git_hash}");
    println!("cargo:rerun-if-changed={}", PathBuf::from(".git/index").display());

    let defs_dir = env::var_os("RIVE_DEFS_DIR").map(PathBuf::from)
        .or_else(|| ["rive-cpp/dev/defs", "rive-rs/submodules/rive-cpp/dev/defs"]
        .into_iter().map(PathBuf::from).find(|path| path.is_dir()))
        .unwrap_or_else(|| panic!("Rive definitions not found; \
            set RIVE_DEFS_DIR or checkout rive-cpp/dev/defs"));
    println!("cargo:rerun-if-changed={}", defs_dir.display());

    /*let output = PathBuf::from(env::var_os("OUT_DIR")
    //    .expect("Cargo did not set OUT_DIR")).join("rive_defs.rs");
    let output = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or("".into()))
        .join("target").join("rive_defs.rs"); // avoid redundant with OUT_DIR
    println!("cargo:rustc-env=RIVE_DEFS_RS={}", output.display()); */
    let output = PathBuf::from("target/rive_defs.rs");
    parse_rive_defs::generate(&defs_dir, &output)
        .unwrap_or_else(|error| panic!("failed to generate {}: {error}", output.display()));
}

#[path = "src/rive/parse_defs.rs"] mod parse_rive_defs;
