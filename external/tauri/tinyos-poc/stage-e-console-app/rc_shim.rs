//! Build-tooling stub, not part of the measured fork diff — the `vswhom-sys` pattern
//! (see `tinyos-poc/Cargo.toml`) applied to the Windows Resource Compiler.
//!
//! `tauri-build` → `tauri-winres` → `embed-resource` insists on an `rc.exe`, whose sole
//! job here would be embedding an icon and version block into the exe. This machine
//! deliberately carries no Microsoft tooling (rust-lld + cargo-xwin splat), so this shim
//! answers the same call (`rc /fo <out> /I <dir> <in.rc>`) by emitting a *valid empty*
//! `.res` — the 32-byte null resource entry — which rust-lld links happily. The console
//! exe simply ships without icon/version resources; nothing Stage E measures cares.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .windows(2)
        .find(|pair| pair[0] == "/fo")
        .map(|pair| pair[1].clone())
        .expect("rc-shim: expected a /fo <out.res> argument");
    // The canonical empty resource file: one 32-byte null entry (DataSize 0,
    // HeaderSize 0x20, type 0xFFFF/0, name 0xFFFF/0, everything else zero).
    let mut empty = [0u8; 32];
    empty[4] = 0x20;
    empty[8] = 0xff;
    empty[9] = 0xff;
    empty[12] = 0xff;
    empty[13] = 0xff;
    std::fs::write(&out, empty).expect("rc-shim: cannot write the empty .res");
}
