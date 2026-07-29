//! Stub of `vswhom-sys` 0.1.3 with the identical public surface and no C code.
//!
//! The real crate compiles a C shim purely to *locate* a Visual Studio install. This machine
//! builds MSVC-target binaries with `rust-lld` and the cargo-xwin splat and has no VS to find,
//! so the honest answer — nothing found — is returned without needing `cl.exe` to say it.
//! Nothing in the PoC build path ever calls these (they would only run if `embed-resource`
//! had to locate `rc.exe` for a Windows resource script, which the vendored-crate builds do
//! not use); the stub exists so the *build* of the dependency graph does not require the very
//! toolchain whose absence it would report.

#![allow(non_camel_case_types)]

use libc::{c_int, wchar_t};
use std::ptr;

#[repr(C)]
pub struct Find_Result {
    pub windows_sdk_version: c_int, // 0 = not found
    pub windows_sdk_root: *mut wchar_t,
    pub windows_sdk_um_library_path: *mut wchar_t,
    pub windows_sdk_ucrt_library_path: *mut wchar_t,
    pub vs_exe_path: *mut wchar_t,
    pub vs_library_path: *mut wchar_t,
}

pub unsafe fn vswhom_find_visual_studio_and_windows_sdk() -> Find_Result {
    Find_Result {
        windows_sdk_version: 0,
        windows_sdk_root: ptr::null_mut(),
        windows_sdk_um_library_path: ptr::null_mut(),
        windows_sdk_ucrt_library_path: ptr::null_mut(),
        vs_exe_path: ptr::null_mut(),
        vs_library_path: ptr::null_mut(),
    }
}

pub unsafe fn vswhom_free_resources(_result: *mut Find_Result) {}
