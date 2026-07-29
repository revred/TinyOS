fn main() {
    // No Microsoft tooling on this machine (see rc_shim.rs): compile the rc shim and
    // point `embed-resource`'s documented `RC` override at it before tauri-build runs.
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set for build scripts");
    let shim = format!("{out_dir}/rc-shim.exe");
    let status = std::process::Command::new("rustc")
        .args(["rc_shim.rs", "-Clinker=rust-lld", "-o", &shim])
        .status()
        .expect("rustc must be invocable to build the rc shim");
    assert!(status.success(), "rc shim failed to compile");
    std::env::set_var("RC", &shim);
    println!("cargo:rerun-if-changed=rc_shim.rs");

    tauri_build::build()
}
