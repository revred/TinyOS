//! Generator for `capability-probe.txe` (`STORY-P1-03-03`) — a genuine,
//! minimal PE32+ image whose only job is to *call an allowlisted Win32 API
//! and prove it returned the right answer*.
//!
//! **Why this exists.** `STORY-P1-03-02` scheduled `blue-sharc.exe` and
//! contained it, but that image faults on its very first import call, so it
//! can never demonstrate the other half of the capability contract: that a
//! call the policy *grants* actually resolves, executes, and returns a
//! correct value to the caller. Demonstrating that needs an image whose
//! first import is one this shim allowlists — and there is no Windows
//! toolchain in this workspace or in CI to compile one. So this module
//! emits the bytes directly.
//!
//! **What it is and is not.** Every structure here is real: a real DOS
//! header, a real PE32+ optional header, a real section table, a real import
//! directory with a separate ILT and IAT, and real x86-64 machine code. It
//! is parsed at run time by the same `exec::pe::parse` that reads
//! `blue-sharc.exe`, mapped by the same `AddressSpace::create`, and patched
//! by the same `exec::iat::patch_imports` — no test-only path anywhere. What
//! it is *not* is a compiler's output: the code is hand-assembled, so it
//! exercises the loader and the capability boundary rather than a real
//! toolchain's code generation. `blue-sharc.exe` remains the evidence for
//! *that*, and the two images answer different questions on purpose.
//!
//! **Why generated rather than committed as an opaque blob.** A checked-in
//! binary nobody can regenerate is a fixture nobody can review. This module
//! is the source; `xtask make-probe-pe` is the build step; the committed
//! `.txe` is its output, and the tests below re-derive every offset the
//! image claims so a silent layout mistake fails here rather than as an
//! unexplained triple fault under QEMU.
//!
//! **Sections are page-aligned on disk** (`FileAlignment` ==
//! `SectionAlignment` == 4096), which is exactly the layout `xtask pack-txe`
//! produces for `blue-sharc.exe`, so this image needs no repacking step of
//! its own — see `exec::address_space`'s module doc for why a real linker's
//! 512-byte file alignment cannot be mapped directly.

/// The image's preferred base — above `exec::address_space`'s
/// `KERNEL_RESERVED_REGION_END`, and the same base `blue-sharc.exe` uses.
pub const IMAGE_BASE: u64 = 0x1_4000_0000;

/// Page size, used as both `SectionAlignment` and `FileAlignment`.
const PAGE: u32 = 0x1000;

/// `.text` (R+X): the probe's own code. Also the entry point.
const TEXT_RVA: u32 = 0x1000;
/// `.rdata` (R): import descriptors, ILT, IAT, and the name tables.
const RDATA_RVA: u32 = 0x2000;
/// `.data` (RW): the single slot the probe stores its result into.
const DATA_RVA: u32 = 0x3000;

/// Where the probe writes `GetCurrentProcess`'s return value. The
/// supervisor reads this back after the task is contained — it is the
/// evidence that the call actually happened and returned correctly, rather
/// than that the task merely ran.
pub const RESULT_RVA: u32 = DATA_RVA;

/// Import Lookup Table (names; immutable).
const ILT_RVA: u32 = RDATA_RVA + 0x40;
/// Import Address Table — the table a loader patches, and the one every
/// call site indirects through. Deliberately separate from the ILT so this
/// image exercises the split-table layout every real linker emits.
pub const IAT_RVA: u32 = RDATA_RVA + 0x60;
const IIBN_GET_CURRENT_PROCESS_RVA: u32 = RDATA_RVA + 0x80;
const IIBN_EXIT_PROCESS_RVA: u32 = RDATA_RVA + 0xA0;
const DLL_NAME_RVA: u32 = RDATA_RVA + 0xC0;

/// Bit 63 of a PE32+ thunk: set when the thunk names an ordinal rather than
/// a symbol. This generator never emits one — every import here is by name —
/// and the test suite asserts exactly that, which is why the constant is
/// referenced only from tests.
#[cfg(test)]
const IMAGE_ORDINAL_FLAG64: u64 = 0x8000_0000_0000_0000;
const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
const IMAGE_SCN_MEM_READ: u32 = 0x4000_0000;
const IMAGE_SCN_MEM_WRITE: u32 = 0x8000_0000;

/// Total image size: four pages (headers, `.text`, `.rdata`, `.data`).
const IMAGE_LEN: usize = 4 * PAGE as usize;

/// The probe's machine code, assembled by hand.
///
/// ```text
///   sub  rsp, 0x28              48 83 EC 28
///   call [rip + iat+0]          FF 15 <disp32>   ; KERNEL32!GetCurrentProcess
///   mov  [rip + result], rax    48 89 05 <disp32>
///   call [rip + iat+8]          FF 15 <disp32>   ; KERNEL32!ExitProcess
///   hlt                         F4
/// ```
///
/// **The `sub rsp, 0x28` is not padding.** The trampolines this calls are
/// `extern "win64"`, and the Microsoft x64 ABI makes the *caller*
/// responsible for a 32-byte shadow store above the return address that the
/// callee may freely spill into. Omitting it would let a callee's spill
/// overwrite this frame. The extra 8 bytes restore the ABI's alignment rule
/// (`RSP % 16 == 8` on entry to the callee, i.e. 16-byte aligned before the
/// `call` pushes): `Context::new` hands control here with `RSP % 16 == 8`,
/// so subtracting 0x28 lands on 16-aligned, and each `call` then makes it 8
/// again — exactly what a compiler-generated prologue would arrange.
///
/// The final `hlt` is unreachable: `ExitProcess` does not return.
fn text_bytes() -> [u8; PAGE as usize] {
    let mut page = [0u8; PAGE as usize];
    let mut code: Vec<u8> = Vec::new();

    // sub rsp, 0x28
    code.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]);

    // call qword ptr [rip + disp32] -> IAT slot 0 (GetCurrentProcess).
    // `disp32` is measured from the address of the *next* instruction.
    code.extend_from_slice(&[0xFF, 0x15]);
    let next = TEXT_RVA + code.len() as u32 + 4;
    code.extend_from_slice(&(IAT_RVA.wrapping_sub(next)).to_le_bytes());

    // mov qword ptr [rip + disp32], rax  — store the returned handle.
    code.extend_from_slice(&[0x48, 0x89, 0x05]);
    let next = TEXT_RVA + code.len() as u32 + 4;
    code.extend_from_slice(&(RESULT_RVA.wrapping_sub(next)).to_le_bytes());

    // call qword ptr [rip + disp32] -> IAT slot 1 (ExitProcess).
    code.extend_from_slice(&[0xFF, 0x15]);
    let next = TEXT_RVA + code.len() as u32 + 4;
    code.extend_from_slice(&((IAT_RVA + 8).wrapping_sub(next)).to_le_bytes());

    // hlt — never reached.
    code.push(0xF4);

    page[..code.len()].copy_from_slice(&code);
    page
}

/// `.rdata`: one import descriptor for `KERNEL32.dll` importing
/// `GetCurrentProcess` then `ExitProcess`, with a separate ILT and IAT.
fn rdata_bytes() -> [u8; PAGE as usize] {
    let mut page = [0u8; PAGE as usize];
    let put = |page: &mut [u8; PAGE as usize], rva: u32, bytes: &[u8]| {
        let offset = (rva - RDATA_RVA) as usize;
        page[offset..offset + bytes.len()].copy_from_slice(bytes);
    };

    // Import descriptor: OriginalFirstThunk, TimeDateStamp, ForwarderChain,
    // Name, FirstThunk — then the all-zero terminating descriptor, which the
    // zeroed page already provides.
    let mut descriptor = Vec::new();
    descriptor.extend_from_slice(&ILT_RVA.to_le_bytes());
    descriptor.extend_from_slice(&0u32.to_le_bytes());
    descriptor.extend_from_slice(&0u32.to_le_bytes());
    descriptor.extend_from_slice(&DLL_NAME_RVA.to_le_bytes());
    descriptor.extend_from_slice(&IAT_RVA.to_le_bytes());
    put(&mut page, RDATA_RVA, &descriptor);

    // ILT and IAT hold the same unpatched values before load: the RVA of
    // each import's IMAGE_IMPORT_BY_NAME record. Overwriting the IAT copy
    // is the loader's job (`exec::iat`).
    for (index, iibn) in [IIBN_GET_CURRENT_PROCESS_RVA, IIBN_EXIT_PROCESS_RVA].iter().enumerate() {
        let thunk = u64::from(*iibn).to_le_bytes();
        put(&mut page, ILT_RVA + (index as u32 * 8), &thunk);
        put(&mut page, IAT_RVA + (index as u32 * 8), &thunk);
    }
    // The terminating zero thunks are already present in the zeroed page.

    // IMAGE_IMPORT_BY_NAME records: a u16 hint then the NUL-terminated name.
    put(&mut page, IIBN_GET_CURRENT_PROCESS_RVA, &[0, 0]);
    put(&mut page, IIBN_GET_CURRENT_PROCESS_RVA + 2, b"GetCurrentProcess\0");
    put(&mut page, IIBN_EXIT_PROCESS_RVA, &[0, 0]);
    put(&mut page, IIBN_EXIT_PROCESS_RVA + 2, b"ExitProcess\0");
    put(&mut page, DLL_NAME_RVA, b"KERNEL32.dll\0");

    page
}

/// Builds the complete image.
pub fn build() -> Vec<u8> {
    let mut image = vec![0u8; IMAGE_LEN];

    // -- DOS header --
    image[0..2].copy_from_slice(b"MZ");
    image[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes()); // e_lfanew

    // -- PE signature + COFF header --
    let pe = 0x80usize;
    image[pe..pe + 4].copy_from_slice(b"PE\0\0");
    let coff = pe + 4;
    image[coff..coff + 2].copy_from_slice(&0x8664u16.to_le_bytes()); // AMD64
    image[coff + 2..coff + 4].copy_from_slice(&3u16.to_le_bytes()); // NumberOfSections
    image[coff + 16..coff + 18].copy_from_slice(&240u16.to_le_bytes()); // SizeOfOptionalHeader
    image[coff + 18..coff + 20].copy_from_slice(&0x0022u16.to_le_bytes()); // EXECUTABLE | LARGE_ADDRESS_AWARE

    // -- Optional header (PE32+) --
    let opt = coff + 20;
    image[opt..opt + 2].copy_from_slice(&0x20bu16.to_le_bytes()); // PE32+ magic
    image[opt + 16..opt + 20].copy_from_slice(&TEXT_RVA.to_le_bytes()); // AddressOfEntryPoint
    image[opt + 20..opt + 24].copy_from_slice(&TEXT_RVA.to_le_bytes()); // BaseOfCode
    image[opt + 24..opt + 32].copy_from_slice(&IMAGE_BASE.to_le_bytes()); // ImageBase
    image[opt + 32..opt + 36].copy_from_slice(&PAGE.to_le_bytes()); // SectionAlignment
    image[opt + 36..opt + 40].copy_from_slice(&PAGE.to_le_bytes()); // FileAlignment
    image[opt + 56..opt + 60].copy_from_slice(&(IMAGE_LEN as u32).to_le_bytes()); // SizeOfImage
    image[opt + 60..opt + 64].copy_from_slice(&PAGE.to_le_bytes()); // SizeOfHeaders
    image[opt + 68..opt + 70].copy_from_slice(&3u16.to_le_bytes()); // Subsystem: CONSOLE
    image[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes()); // NumberOfRvaAndSizes
                                                                       // Data directory 1: the import table.
    let import_dir = opt + 112 + 8;
    image[import_dir..import_dir + 4].copy_from_slice(&RDATA_RVA.to_le_bytes());
    image[import_dir + 4..import_dir + 8].copy_from_slice(&40u32.to_le_bytes());

    // -- Section table --
    let sections: [(&[u8], u32, u32); 3] = [
        (b".text", TEXT_RVA, IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_EXECUTE),
        (b".rdata", RDATA_RVA, IMAGE_SCN_MEM_READ),
        (b".data", DATA_RVA, IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE),
    ];
    let table = opt + 240;
    for (index, (name, rva, characteristics)) in sections.iter().enumerate() {
        let header = table + index * 40;
        image[header..header + name.len()].copy_from_slice(name);
        image[header + 8..header + 12].copy_from_slice(&PAGE.to_le_bytes()); // VirtualSize
        image[header + 12..header + 16].copy_from_slice(&rva.to_le_bytes()); // VirtualAddress
        image[header + 16..header + 20].copy_from_slice(&PAGE.to_le_bytes()); // SizeOfRawData
                                                                              // Page-aligned on disk, so the file offset is the RVA (see the
                                                                              // module doc for why this matters to `AddressSpace::create`).
        image[header + 20..header + 24].copy_from_slice(&rva.to_le_bytes()); // PointerToRawData
        image[header + 36..header + 40].copy_from_slice(&characteristics.to_le_bytes());
    }

    image[TEXT_RVA as usize..(TEXT_RVA + PAGE) as usize].copy_from_slice(&text_bytes());
    image[RDATA_RVA as usize..(RDATA_RVA + PAGE) as usize].copy_from_slice(&rdata_bytes());
    // `.data` stays zeroed: the result slot starts empty, so a supervisor
    // reading the expected value back can never be reading a value the
    // image shipped with.

    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_image_is_four_pages_and_starts_with_a_dos_header() {
        let image = build();
        assert_eq!(image.len(), IMAGE_LEN);
        assert_eq!(&image[0..2], b"MZ");
        assert_eq!(&image[0x80..0x84], b"PE\0\0");
    }

    // The result slot must start zero, or "the probe stored the right
    // value" could be satisfied by a value the image already contained.
    #[test]
    fn the_result_slot_starts_zeroed() {
        let image = build();
        let offset = RESULT_RVA as usize;
        assert_eq!(&image[offset..offset + 8], &[0u8; 8]);
    }

    // Every RIP-relative displacement is re-derived here from the encoded
    // bytes, so a hand-assembly slip is a failing test rather than a
    // triple fault under QEMU with no diagnostic.
    #[test]
    fn the_hand_assembled_displacements_resolve_to_their_intended_targets() {
        let image = build();
        let text = &image[TEXT_RVA as usize..];

        assert_eq!(&text[0..4], &[0x48, 0x83, 0xEC, 0x28], "sub rsp, 0x28");

        // call [rip+d] at RVA 0x1004, 6 bytes, next instruction at 0x100A.
        assert_eq!(&text[4..6], &[0xFF, 0x15]);
        let d1 = i32::from_le_bytes(text[6..10].try_into().unwrap());
        assert_eq!((TEXT_RVA + 10).wrapping_add(d1 as u32), IAT_RVA, "first call targets IAT[0]");

        // mov [rip+d], rax at RVA 0x100A, 7 bytes, next at 0x1011.
        assert_eq!(&text[10..13], &[0x48, 0x89, 0x05]);
        let d2 = i32::from_le_bytes(text[13..17].try_into().unwrap());
        assert_eq!(
            (TEXT_RVA + 17).wrapping_add(d2 as u32),
            RESULT_RVA,
            "the store targets the result slot"
        );

        // call [rip+d] at RVA 0x1011, 6 bytes, next at 0x1017.
        assert_eq!(&text[17..19], &[0xFF, 0x15]);
        let d3 = i32::from_le_bytes(text[19..23].try_into().unwrap());
        assert_eq!(
            (TEXT_RVA + 23).wrapping_add(d3 as u32),
            IAT_RVA + 8,
            "second call targets IAT[1]"
        );

        assert_eq!(text[23], 0xF4, "hlt");
    }

    // The ILT and IAT are genuinely separate tables holding the same
    // pre-load values — the layout that makes patching the wrong one a
    // detectable mistake.
    #[test]
    fn the_ilt_and_iat_are_separate_tables_with_matching_unpatched_thunks() {
        let image = build();
        assert_ne!(ILT_RVA, IAT_RVA);
        for index in 0..2u32 {
            let ilt = ILT_RVA as usize + (index as usize * 8);
            let iat = IAT_RVA as usize + (index as usize * 8);
            assert_eq!(&image[ilt..ilt + 8], &image[iat..iat + 8]);
            let thunk = u64::from_le_bytes(image[iat..iat + 8].try_into().unwrap());
            assert_ne!(thunk, 0);
            assert_eq!(thunk & IMAGE_ORDINAL_FLAG64, 0, "both imports are by name");
        }
        // Terminators.
        let ilt_end = ILT_RVA as usize + 16;
        assert_eq!(&image[ilt_end..ilt_end + 8], &[0u8; 8]);
        let iat_end = IAT_RVA as usize + 16;
        assert_eq!(&image[iat_end..iat_end + 8], &[0u8; 8]);
    }

    // Both imported names are spelled exactly as `win32_shim::resolve`
    // matches them — a typo here would silently become a trapped import
    // and the probe would prove the opposite of what it claims.
    #[test]
    fn the_imported_names_match_the_allowlist_spellings() {
        let image = build();
        let name_at = |rva: u32| {
            let start = rva as usize + 2;
            let end = start + image[start..].iter().position(|&b| b == 0).unwrap();
            std::str::from_utf8(&image[start..end]).unwrap().to_string()
        };
        assert_eq!(name_at(IIBN_GET_CURRENT_PROCESS_RVA), "GetCurrentProcess");
        assert_eq!(name_at(IIBN_EXIT_PROCESS_RVA), "ExitProcess");
        let dll_start = DLL_NAME_RVA as usize;
        let dll_end = dll_start + image[dll_start..].iter().position(|&b| b == 0).unwrap();
        assert_eq!(std::str::from_utf8(&image[dll_start..dll_end]).unwrap(), "KERNEL32.dll");
    }

    // Section headers describe page-aligned, directly-mappable sections
    // with the permissions the W^X loader will enforce.
    #[test]
    fn sections_are_page_aligned_and_carry_distinct_permissions() {
        let image = build();
        let table = 0x80 + 4 + 20 + 240;
        let expected = [
            (".text", TEXT_RVA, IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_EXECUTE),
            (".rdata", RDATA_RVA, IMAGE_SCN_MEM_READ),
            (".data", DATA_RVA, IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE),
        ];
        for (index, (name, rva, characteristics)) in expected.iter().enumerate() {
            let header = table + index * 40;
            let end = header + image[header..header + 8].iter().position(|&b| b == 0).unwrap();
            assert_eq!(std::str::from_utf8(&image[header..end]).unwrap(), *name);
            let virtual_address =
                u32::from_le_bytes(image[header + 12..header + 16].try_into().unwrap());
            let raw_pointer =
                u32::from_le_bytes(image[header + 20..header + 24].try_into().unwrap());
            assert_eq!(virtual_address, *rva);
            assert_eq!(raw_pointer, *rva, "file offset must equal RVA (page-aligned on disk)");
            assert_eq!(
                u32::from_le_bytes(image[header + 36..header + 40].try_into().unwrap()),
                *characteristics
            );
        }
        // No section is both writable and executable — the image itself
        // must not ask for something the W^X loader would have to refuse.
        for index in 0..3 {
            let header = table + index * 40;
            let characteristics =
                u32::from_le_bytes(image[header + 36..header + 40].try_into().unwrap());
            let writable = characteristics & IMAGE_SCN_MEM_WRITE != 0;
            let executable = characteristics & IMAGE_SCN_MEM_EXECUTE != 0;
            assert!(!(writable && executable), "section {index} requests W+X");
        }
    }

    #[test]
    fn generation_is_deterministic() {
        assert_eq!(build(), build());
    }
}
