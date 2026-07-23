
fn main() {     // https://doc.rust-lang.org/stable/cargo/reference/build-scripts.html
    //println!("cargo:rerun-if-changed=build.rs");    // XXX: prevent re-run indead
    // By default, cargo always re-run the build script if any file within the package
    // is changed, and no any rerun-if instruction is emitted.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}",
        chrono::Local::now().format("%H:%M:%S%z %Y-%m-%d"));

    let git_hash = std::process::Command::new("git").args(["rev-parse", "--short", "HEAD"])
        .output().ok().filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|hash| hash.trim().to_owned()).filter(|hash| !hash.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=BUILD_GIT_HASH={git_hash}");
    println!("cargo:rerun-if-changed={}", std::path::PathBuf::from(".git/index").display());
}
