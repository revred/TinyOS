//! PE64 (`PE32+`) executable image parsing into a validated, typed
//! [`LoadDescriptor`] (`STORY-P0-05-01`, `TEST-P0-05-01-A`, Goals `G-PC-1`
//! and `G-PC-4`).
//!
//! A PE file is untrusted input from the kernel's own trust-boundary
//! perspective — externally supplied, potentially adversarial — so this
//! module applies the same discipline `hal_x86_64::acpi` already established
//! for untrusted ACPI tables: every offset and length the file claims is
//! checked against the file's actual size before it's trusted, and parsing
//! fails closed with a typed [`PeError`] on anything malformed or truncated
//! rather than reading past a declared bound or panicking.
//!
//! This module is 100% safe, pure Rust: it operates on an already-obtained
//! `&[u8]` (the whole file, or a validated prefix) and performs no I/O and
//! no `unsafe`, so it is fully host-testable with hand-crafted and (once
//! sourced, per `STORY-P0-05-04`) real `blue-sharc.exe` fixture bytes,
//! without a QEMU round-trip.
//!
//! Only PE32+ (64-bit) images are supported — the flagship validation case
//! (`blue-sharc.exe`) and every Goal `G-PC-1`..`G-PC-4` target only 64-bit
//! executables, so a 32-bit (`PE32`) image is rejected rather than
//! partially supported.

/// Errors this module fails closed with rather than reading past a
/// file-declared boundary or panicking on malformed input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeError {
    /// A fixed-size field or table would read past the end of the file.
    Truncated,
    /// The DOS header's `"MZ"` signature is missing.
    InvalidDosSignature,
    /// The DOS header's `e_lfanew` field points outside the file.
    InvalidPeHeaderOffset,
    /// The `"PE\0\0"` signature at `e_lfanew` is missing.
    InvalidPeSignature,
    /// The COFF header's `Machine` field is not `IMAGE_FILE_MACHINE_AMD64`.
    UnsupportedMachine,
    /// The optional header's magic is not `IMAGE_NT_OPTIONAL_HDR64_MAGIC`
    /// (`0x20b`) — this module only supports PE32+ images.
    NotPe32Plus,
    /// The section table (`NumberOfSections` entries of 40 bytes each,
    /// immediately after the optional header) runs past the file's end.
    SectionTableOutOfBounds,
    /// A section's declared file offset and size run past the file's end.
    SectionDataOutOfBounds,
    /// A section requests both write and execute permission (`G-PC-1`'s W^X
    /// requirement, enforced at parse time).
    WriteExecuteSection,
    /// More sections were found than the caller's `SECTIONS` capacity
    /// allows.
    TooManySections,
    /// An RVA (import directory, DLL name, or imported symbol name) does
    /// not fall inside any parsed section.
    RvaOutOfBounds,
    /// A DLL or symbol name has no NUL terminator within the file.
    NameOutOfBounds,
    /// A DLL or symbol name exceeds this module's fixed buffer capacity.
    NameTooLong,
    /// More imported (DLL, symbol) pairs were found than the caller's
    /// `IMPORTS` capacity allows.
    TooManyImports,
}

/// Section read/write/execute permissions, as declared by a PE section's
/// `Characteristics` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    /// `IMAGE_SCN_MEM_READ`.
    pub read: bool,
    /// `IMAGE_SCN_MEM_WRITE`.
    pub write: bool,
    /// `IMAGE_SCN_MEM_EXECUTE`.
    pub execute: bool,
}

/// One PE section, validated against the file's actual size and W^X at
/// parse time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionDescriptor {
    /// Relative virtual address (RVA) this section is mapped at.
    pub virtual_address: u32,
    /// Size in memory once mapped (may exceed `file_size`, e.g. `.bss`).
    pub virtual_size: u32,
    /// Byte offset of this section's raw data within the file.
    pub file_offset: u32,
    /// Size in bytes of this section's raw data within the file.
    pub file_size: u32,
    /// Permissions this section is mapped with.
    pub permissions: Permissions,
}

/// Maximum length of a DLL name this module records, in bytes (excluding
/// the NUL terminator). A name at or beyond this length fails closed with
/// [`PeError::NameTooLong`] rather than being truncated silently.
pub const MAX_DLL_NAME_LEN: usize = 64;

/// Maximum length of an imported symbol name this module records, in bytes
/// (excluding the NUL terminator). See [`MAX_DLL_NAME_LEN`].
pub const MAX_SYMBOL_NAME_LEN: usize = 128;

/// A fixed-capacity byte buffer holding a name of at most `N` bytes, with no
/// heap allocation — the `no_std` equivalent of a bounded `String`.
#[derive(Clone, Copy)]
pub struct FixedBytes<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> FixedBytes<N> {
    /// The name's bytes (excluding any NUL terminator).
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    /// Test-only constructor for other modules in this crate (e.g.
    /// `win32_shim`'s tests) that need to build an [`ImportEntry`] by hand
    /// without going through [`parse`]'s full PE byte layout.
    #[cfg(test)]
    pub(crate) fn for_test(bytes: [u8; N], len: usize) -> Self {
        FixedBytes { bytes, len }
    }
}

impl<const N: usize> PartialEq for FixedBytes<N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<const N: usize> Eq for FixedBytes<N> {}

impl<const N: usize> core::fmt::Debug for FixedBytes<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match core::str::from_utf8(self.as_bytes()) {
            Ok(s) => f.debug_tuple("FixedBytes").field(&s).finish(),
            Err(_) => f.debug_tuple("FixedBytes").field(&self.as_bytes()).finish(),
        }
    }
}

/// One imported (DLL name, imported symbol name) pair, per `STORY-P0-05-01`
/// acceptance criterion 3 — resolution against an allowlist is
/// `STORY-P0-05-03`'s job, not this module's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportEntry {
    /// The importing DLL's name, e.g. `b"KERNEL32.dll"`.
    pub dll_name: FixedBytes<MAX_DLL_NAME_LEN>,
    /// The imported symbol's name, e.g. `b"HeapAlloc"`.
    pub symbol_name: FixedBytes<MAX_SYMBOL_NAME_LEN>,
}

/// A parsed, validated PE64 image: its entry point, image base, sections,
/// and named imports.
///
/// `SECTIONS` and `IMPORTS` are caller-chosen capacity bounds (analogous to
/// `Pool<T, N>` in `kernel::mem` and `hal::topology::Topology<N>`) — a file
/// declaring more sections or imports than fit fails closed via
/// [`PeError::TooManySections`]/[`PeError::TooManyImports`] rather than
/// growing unbounded storage.
#[derive(Debug, Clone, Copy)]
pub struct LoadDescriptor<const SECTIONS: usize, const IMPORTS: usize> {
    /// Relative virtual address of the image's entry point.
    pub entry_point_rva: u32,
    /// The image's preferred base virtual address.
    pub image_base: u64,
    sections: [Option<SectionDescriptor>; SECTIONS],
    section_count: usize,
    imports: [Option<ImportEntry>; IMPORTS],
    import_count: usize,
}

impl<const SECTIONS: usize, const IMPORTS: usize> LoadDescriptor<SECTIONS, IMPORTS> {
    /// Iterates over this image's sections in file order.
    pub fn sections(&self) -> impl Iterator<Item = &SectionDescriptor> {
        self.sections[..self.section_count].iter().filter_map(Option::as_ref)
    }

    /// Iterates over this image's imported (DLL, symbol) pairs, in the
    /// order they were declared.
    pub fn imports(&self) -> impl Iterator<Item = &ImportEntry> {
        self.imports[..self.import_count].iter().filter_map(Option::as_ref)
    }
}

const DOS_HEADER_LEN: usize = 64;
const E_LFANEW_OFFSET: usize = 0x3C;
const PE_SIGNATURE: [u8; 4] = *b"PE\0\0";
const COFF_HEADER_LEN: usize = 20;
/// `IMAGE_FILE_MACHINE_AMD64`.
const MACHINE_AMD64: u16 = 0x8664;
/// `IMAGE_NT_OPTIONAL_HDR64_MAGIC` — identifies a PE32+ optional header.
const OPTIONAL_HEADER_PE32_PLUS_MAGIC: u16 = 0x20b;
const SECTION_HEADER_LEN: usize = 40;
const IMPORT_DESCRIPTOR_LEN: usize = 20;
/// Size of one Import (Lookup/Address) Table thunk entry in a PE32+ image.
const THUNK_LEN: usize = 8;
const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
const IMAGE_SCN_MEM_WRITE: u32 = 0x8000_0000;
const IMAGE_SCN_MEM_READ: u32 = 0x4000_0000;
/// Set on a 64-bit thunk when it names an ordinal rather than a symbol
/// name; such imports are not recorded ([`LoadDescriptor::imports`] only
/// carries named imports — `blue-sharc.exe`'s expected import table is
/// name-based, and an allowlist match against `STORY-P0-05-03` is only
/// meaningful for named imports).
const IMAGE_ORDINAL_FLAG64: u64 = 0x8000_0000_0000_0000;
/// Byte offset of the Import Directory entry within the optional header's
/// Data Directory array (index 1 of 16, each entry 8 bytes, array starts at
/// offset 112 in a PE32+ optional header).
const IMPORT_DIRECTORY_ENTRY_OFFSET: usize = 112 + 8;

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, PeError> {
    let end = offset.checked_add(2).ok_or(PeError::Truncated)?;
    let slice = bytes.get(offset..end).ok_or(PeError::Truncated)?;
    Ok(u16::from_le_bytes(slice.try_into().unwrap()))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, PeError> {
    let end = offset.checked_add(4).ok_or(PeError::Truncated)?;
    let slice = bytes.get(offset..end).ok_or(PeError::Truncated)?;
    Ok(u32::from_le_bytes(slice.try_into().unwrap()))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, PeError> {
    let end = offset.checked_add(8).ok_or(PeError::Truncated)?;
    let slice = bytes.get(offset..end).ok_or(PeError::Truncated)?;
    Ok(u64::from_le_bytes(slice.try_into().unwrap()))
}

/// Reads a NUL-terminated byte string starting at `offset`, failing closed
/// if the terminator isn't found within the file or the name exceeds `N`
/// bytes.
fn read_c_string<const N: usize>(bytes: &[u8], offset: usize) -> Result<FixedBytes<N>, PeError> {
    let mut buf = [0u8; N];
    let mut len = 0usize;
    let mut i = offset;
    loop {
        let b = *bytes.get(i).ok_or(PeError::NameOutOfBounds)?;
        if b == 0 {
            break;
        }
        if len >= N {
            return Err(PeError::NameTooLong);
        }
        buf[len] = b;
        len += 1;
        i += 1;
    }
    Ok(FixedBytes { bytes: buf, len })
}

/// Translates a relative virtual address into a file byte offset by
/// finding the section whose mapped range contains it. Uses
/// `max(virtual_size, file_size)` as the range's extent, matching real PE
/// loaders' tolerance of headers where the two disagree.
fn rva_to_file_offset(sections: &[Option<SectionDescriptor>], rva: u32) -> Option<usize> {
    for section in sections.iter().filter_map(Option::as_ref) {
        let extent = section.virtual_size.max(section.file_size);
        let end = section.virtual_address.checked_add(extent)?;
        if rva >= section.virtual_address && rva < end {
            let delta = rva - section.virtual_address;
            return Some(section.file_offset as usize + delta as usize);
        }
    }
    None
}

/// Parses a PE32+ image from `bytes` (the whole file, or a validated
/// prefix) into a [`LoadDescriptor`], failing closed with a typed
/// [`PeError`] on any malformed, truncated, or W^X-violating input rather
/// than reading past a declared bound or panicking. Pure and
/// deterministic: parsing the same bytes twice yields the same result, and
/// a rejection never partially constructs a descriptor.
pub fn parse<const SECTIONS: usize, const IMPORTS: usize>(
    bytes: &[u8],
) -> Result<LoadDescriptor<SECTIONS, IMPORTS>, PeError> {
    if bytes.len() < DOS_HEADER_LEN || bytes[0..2] != *b"MZ" {
        return Err(PeError::InvalidDosSignature);
    }
    let e_lfanew = read_u32(bytes, E_LFANEW_OFFSET)? as usize;
    let pe_header_end = e_lfanew.checked_add(4).ok_or(PeError::InvalidPeHeaderOffset)?;
    let signature = bytes.get(e_lfanew..pe_header_end).ok_or(PeError::InvalidPeHeaderOffset)?;
    if signature != PE_SIGNATURE {
        return Err(PeError::InvalidPeSignature);
    }

    let coff_offset = pe_header_end;
    let coff_end = coff_offset.checked_add(COFF_HEADER_LEN).ok_or(PeError::Truncated)?;
    if coff_end > bytes.len() {
        return Err(PeError::Truncated);
    }
    if read_u16(bytes, coff_offset)? != MACHINE_AMD64 {
        return Err(PeError::UnsupportedMachine);
    }
    let number_of_sections = read_u16(bytes, coff_offset + 2)? as usize;
    let size_of_optional_header = read_u16(bytes, coff_offset + 16)? as usize;

    let optional_header_offset = coff_end;
    let optional_header_end =
        optional_header_offset.checked_add(size_of_optional_header).ok_or(PeError::Truncated)?;
    if optional_header_end > bytes.len() {
        return Err(PeError::Truncated);
    }
    // Need at least through `NumberOfRvaAndSizes` (offset 108, 4 bytes).
    if size_of_optional_header < 112 {
        return Err(PeError::Truncated);
    }
    if read_u16(bytes, optional_header_offset)? != OPTIONAL_HEADER_PE32_PLUS_MAGIC {
        return Err(PeError::NotPe32Plus);
    }
    let entry_point_rva = read_u32(bytes, optional_header_offset + 16)?;
    let image_base = read_u64(bytes, optional_header_offset + 24)?;
    let number_of_rva_and_sizes = read_u32(bytes, optional_header_offset + 108)? as usize;

    // The import data-directory entry is only present if the optional
    // header actually allocated room for it; `number_of_rva_and_sizes`
    // claiming more entries than the header has room for doesn't widen the
    // read, since `dd_offset + 8` is still checked against
    // `optional_header_end` below.
    let dd_offset = optional_header_offset + IMPORT_DIRECTORY_ENTRY_OFFSET;
    let (import_dir_rva, import_dir_size) = if number_of_rva_and_sizes > 1
        && dd_offset.checked_add(8).is_some_and(|end| end <= optional_header_end)
    {
        (read_u32(bytes, dd_offset)?, read_u32(bytes, dd_offset + 4)?)
    } else {
        (0, 0)
    };

    let section_table_offset = optional_header_end;
    let section_table_len = number_of_sections
        .checked_mul(SECTION_HEADER_LEN)
        .ok_or(PeError::SectionTableOutOfBounds)?;
    let section_table_end = section_table_offset
        .checked_add(section_table_len)
        .ok_or(PeError::SectionTableOutOfBounds)?;
    if section_table_end > bytes.len() {
        return Err(PeError::SectionTableOutOfBounds);
    }

    let mut sections: [Option<SectionDescriptor>; SECTIONS] = [None; SECTIONS];
    let mut section_count = 0usize;
    for i in 0..number_of_sections {
        let off = section_table_offset + i * SECTION_HEADER_LEN;
        let virtual_size = read_u32(bytes, off + 8)?;
        let virtual_address = read_u32(bytes, off + 12)?;
        let size_of_raw_data = read_u32(bytes, off + 16)?;
        let pointer_to_raw_data = read_u32(bytes, off + 20)?;
        let characteristics = read_u32(bytes, off + 36)?;

        if size_of_raw_data > 0 {
            let data_end = (pointer_to_raw_data as usize)
                .checked_add(size_of_raw_data as usize)
                .ok_or(PeError::SectionDataOutOfBounds)?;
            if data_end > bytes.len() {
                return Err(PeError::SectionDataOutOfBounds);
            }
        }

        let permissions = Permissions {
            read: characteristics & IMAGE_SCN_MEM_READ != 0,
            write: characteristics & IMAGE_SCN_MEM_WRITE != 0,
            execute: characteristics & IMAGE_SCN_MEM_EXECUTE != 0,
        };
        if permissions.write && permissions.execute {
            return Err(PeError::WriteExecuteSection);
        }

        if section_count >= SECTIONS {
            return Err(PeError::TooManySections);
        }
        sections[section_count] = Some(SectionDescriptor {
            virtual_address,
            virtual_size,
            file_offset: pointer_to_raw_data,
            file_size: size_of_raw_data,
            permissions,
        });
        section_count += 1;
    }

    let mut imports: [Option<ImportEntry>; IMPORTS] = [None; IMPORTS];
    let mut import_count = 0usize;
    if import_dir_rva != 0 && import_dir_size != 0 {
        let parsed_sections = &sections[..section_count];
        let mut desc_offset =
            rva_to_file_offset(parsed_sections, import_dir_rva).ok_or(PeError::RvaOutOfBounds)?;
        loop {
            let entry_end =
                desc_offset.checked_add(IMPORT_DESCRIPTOR_LEN).ok_or(PeError::Truncated)?;
            if entry_end > bytes.len() {
                return Err(PeError::Truncated);
            }
            let original_first_thunk = read_u32(bytes, desc_offset)?;
            let name_rva = read_u32(bytes, desc_offset + 12)?;
            let first_thunk = read_u32(bytes, desc_offset + 16)?;
            if original_first_thunk == 0 && name_rva == 0 && first_thunk == 0 {
                break;
            }

            let dll_name_offset =
                rva_to_file_offset(parsed_sections, name_rva).ok_or(PeError::RvaOutOfBounds)?;
            let dll_name: FixedBytes<MAX_DLL_NAME_LEN> = read_c_string(bytes, dll_name_offset)?;

            let thunk_rva =
                if original_first_thunk != 0 { original_first_thunk } else { first_thunk };
            if thunk_rva != 0 {
                let mut thunk_offset = rva_to_file_offset(parsed_sections, thunk_rva)
                    .ok_or(PeError::RvaOutOfBounds)?;
                loop {
                    let thunk_end =
                        thunk_offset.checked_add(THUNK_LEN).ok_or(PeError::Truncated)?;
                    if thunk_end > bytes.len() {
                        return Err(PeError::Truncated);
                    }
                    let thunk = read_u64(bytes, thunk_offset)?;
                    if thunk == 0 {
                        break;
                    }
                    if thunk & IMAGE_ORDINAL_FLAG64 == 0 {
                        let hint_name_rva = (thunk & 0xFFFF_FFFF) as u32;
                        let hint_name_offset = rva_to_file_offset(parsed_sections, hint_name_rva)
                            .ok_or(PeError::RvaOutOfBounds)?;
                        // Skip the 2-byte `Hint` field preceding the name.
                        let symbol_name: FixedBytes<MAX_SYMBOL_NAME_LEN> =
                            read_c_string(bytes, hint_name_offset + 2)?;
                        if import_count >= IMPORTS {
                            return Err(PeError::TooManyImports);
                        }
                        imports[import_count] = Some(ImportEntry { dll_name, symbol_name });
                        import_count += 1;
                    }
                    thunk_offset += THUNK_LEN;
                }
            }
            desc_offset += IMPORT_DESCRIPTOR_LEN;
        }
    }

    Ok(LoadDescriptor {
        entry_point_rva,
        image_base,
        sections,
        section_count,
        imports,
        import_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMAGE_BASE_RVA: u32 = 0x1000;

    /// Builds a minimal, well-formed PE32+ image: one `.text` (R+X) section
    /// whose raw data holds one import descriptor (`MYDLL.DLL` importing
    /// `MySymbol`) followed by its null terminator.
    fn well_formed_image() -> std::vec::Vec<u8> {
        // -- import block (mapped at `IMAGE_BASE_RVA`, file offset chosen below) --
        let mut import_block = std::vec::Vec::new();
        // Descriptor 1: OriginalFirstThunk, TimeDateStamp, ForwarderChain, Name, FirstThunk.
        let ilt_rva = IMAGE_BASE_RVA + 40;
        let name_rva = IMAGE_BASE_RVA + 67;
        import_block.extend_from_slice(&ilt_rva.to_le_bytes()); // OriginalFirstThunk
        import_block.extend_from_slice(&0u32.to_le_bytes()); // TimeDateStamp
        import_block.extend_from_slice(&0u32.to_le_bytes()); // ForwarderChain
        import_block.extend_from_slice(&name_rva.to_le_bytes()); // Name
        import_block.extend_from_slice(&ilt_rva.to_le_bytes()); // FirstThunk
        assert_eq!(import_block.len(), 20);
        // Descriptor 2: null terminator.
        import_block.extend_from_slice(&[0u8; 20]);
        assert_eq!(import_block.len(), 40);
        // ILT: one named-import thunk, then a zero terminator.
        let iibn_rva = IMAGE_BASE_RVA + 56;
        import_block.extend_from_slice(&(iibn_rva as u64).to_le_bytes());
        import_block.extend_from_slice(&0u64.to_le_bytes());
        assert_eq!(import_block.len(), 56);
        // IMAGE_IMPORT_BY_NAME: Hint(u16) + symbol name + NUL.
        import_block.extend_from_slice(&0u16.to_le_bytes());
        import_block.extend_from_slice(b"MySymbol\0");
        assert_eq!(import_block.len(), 67);
        // DLL name + NUL.
        import_block.extend_from_slice(b"MYDLL.DLL\0");
        assert_eq!(import_block.len(), 77);

        let section_table_offset = DOS_HEADER_LEN + 4 + COFF_HEADER_LEN + 128;
        let raw_data_offset = section_table_offset + SECTION_HEADER_LEN;

        let mut b = std::vec![0u8; raw_data_offset];
        // DOS header.
        b[0..2].copy_from_slice(b"MZ");
        b[E_LFANEW_OFFSET..E_LFANEW_OFFSET + 4]
            .copy_from_slice(&(DOS_HEADER_LEN as u32).to_le_bytes());
        // PE signature.
        let pe_off = DOS_HEADER_LEN;
        b[pe_off..pe_off + 4].copy_from_slice(&PE_SIGNATURE);
        // COFF header.
        let coff_off = pe_off + 4;
        b[coff_off..coff_off + 2].copy_from_slice(&MACHINE_AMD64.to_le_bytes());
        b[coff_off + 2..coff_off + 4].copy_from_slice(&1u16.to_le_bytes()); // NumberOfSections
        b[coff_off + 16..coff_off + 18].copy_from_slice(&128u16.to_le_bytes()); // SizeOfOptionalHeader
                                                                                // Optional header (PE32+).
        let opt_off = coff_off + COFF_HEADER_LEN;
        b[opt_off..opt_off + 2].copy_from_slice(&OPTIONAL_HEADER_PE32_PLUS_MAGIC.to_le_bytes());
        b[opt_off + 16..opt_off + 20].copy_from_slice(&(IMAGE_BASE_RVA + 4).to_le_bytes()); // AddressOfEntryPoint
        b[opt_off + 24..opt_off + 32].copy_from_slice(&0x1_4000_0000u64.to_le_bytes()); // ImageBase
        b[opt_off + 108..opt_off + 112].copy_from_slice(&2u32.to_le_bytes()); // NumberOfRvaAndSizes
                                                                              // Data directory entry 1 (import).
        let dd1_off = opt_off + IMPORT_DIRECTORY_ENTRY_OFFSET;
        b[dd1_off..dd1_off + 4].copy_from_slice(&IMAGE_BASE_RVA.to_le_bytes());
        b[dd1_off + 4..dd1_off + 8].copy_from_slice(&40u32.to_le_bytes());
        // Section table: one `.text` (R+X) section.
        let sec_off = section_table_offset;
        b[sec_off..sec_off + 5].copy_from_slice(b".text");
        b[sec_off + 8..sec_off + 12].copy_from_slice(&(import_block.len() as u32).to_le_bytes()); // VirtualSize
        b[sec_off + 12..sec_off + 16].copy_from_slice(&IMAGE_BASE_RVA.to_le_bytes()); // VirtualAddress
        b[sec_off + 16..sec_off + 20].copy_from_slice(&(import_block.len() as u32).to_le_bytes()); // SizeOfRawData
        b[sec_off + 20..sec_off + 24].copy_from_slice(&(raw_data_offset as u32).to_le_bytes()); // PointerToRawData
        let characteristics = IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_EXECUTE;
        b[sec_off + 36..sec_off + 40].copy_from_slice(&characteristics.to_le_bytes());

        b.extend_from_slice(&import_block);
        b
    }

    #[test]
    fn parses_a_well_formed_image() {
        let bytes = well_formed_image();
        let descriptor: LoadDescriptor<4, 4> = parse(&bytes).unwrap();
        assert_eq!(descriptor.entry_point_rva, IMAGE_BASE_RVA + 4);
        assert_eq!(descriptor.image_base, 0x1_4000_0000);

        let sections: std::vec::Vec<_> = descriptor.sections().collect();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].virtual_address, IMAGE_BASE_RVA);
        assert_eq!(
            sections[0].permissions,
            Permissions { read: true, write: false, execute: true }
        );

        let imports: std::vec::Vec<_> = descriptor.imports().collect();
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].dll_name.as_bytes(), b"MYDLL.DLL");
        assert_eq!(imports[0].symbol_name.as_bytes(), b"MySymbol");
    }

    #[test]
    fn parsing_is_deterministic() {
        let bytes = well_formed_image();
        let first: LoadDescriptor<4, 4> = parse(&bytes).unwrap();
        let second: LoadDescriptor<4, 4> = parse(&bytes).unwrap();
        assert_eq!(first.entry_point_rva, second.entry_point_rva);
        assert_eq!(first.image_base, second.image_base);
        assert_eq!(
            first.sections().collect::<std::vec::Vec<_>>(),
            second.sections().collect::<std::vec::Vec<_>>()
        );
        assert_eq!(
            first.imports().collect::<std::vec::Vec<_>>(),
            second.imports().collect::<std::vec::Vec<_>>()
        );
    }

    #[test]
    fn rejects_missing_dos_signature() {
        let mut bytes = well_formed_image();
        bytes[0] = b'X';
        let result: Result<LoadDescriptor<4, 4>, PeError> = parse(&bytes);
        assert!(matches!(result, Err(PeError::InvalidDosSignature)));
    }

    #[test]
    fn rejects_missing_pe_signature() {
        let mut bytes = well_formed_image();
        bytes[DOS_HEADER_LEN] = b'X';
        let result: Result<LoadDescriptor<4, 4>, PeError> = parse(&bytes);
        assert!(matches!(result, Err(PeError::InvalidPeSignature)));
    }

    #[test]
    fn rejects_non_amd64_machine() {
        let mut bytes = well_formed_image();
        let coff_off = DOS_HEADER_LEN + 4;
        bytes[coff_off..coff_off + 2].copy_from_slice(&0x14Cu16.to_le_bytes()); // IMAGE_FILE_MACHINE_I386
        let result: Result<LoadDescriptor<4, 4>, PeError> = parse(&bytes);
        assert!(matches!(result, Err(PeError::UnsupportedMachine)));
    }

    #[test]
    fn rejects_truncated_header() {
        let bytes = well_formed_image();
        let truncated = &bytes[..DOS_HEADER_LEN + 4]; // cut off mid-COFF-header
        let result: Result<LoadDescriptor<4, 4>, PeError> = parse(truncated);
        assert!(matches!(result, Err(PeError::Truncated)));
    }

    #[test]
    fn rejects_truncated_section_table() {
        let bytes = well_formed_image();
        let section_table_offset = DOS_HEADER_LEN + 4 + COFF_HEADER_LEN + 128;
        let truncated = &bytes[..section_table_offset + SECTION_HEADER_LEN - 4]; // cut mid-section-entry
        let result: Result<LoadDescriptor<4, 4>, PeError> = parse(truncated);
        assert!(matches!(result, Err(PeError::SectionTableOutOfBounds)));
    }

    #[test]
    fn rejects_section_data_past_end_of_file() {
        let mut bytes = well_formed_image();
        let section_table_offset = DOS_HEADER_LEN + 4 + COFF_HEADER_LEN + 128;
        // Inflate SizeOfRawData so file_offset + size runs past the real
        // file length, without touching the file's actual length.
        let size_off = section_table_offset + 16;
        let inflated = (bytes.len() as u32) + 1000;
        bytes[size_off..size_off + 4].copy_from_slice(&inflated.to_le_bytes());
        let result: Result<LoadDescriptor<4, 4>, PeError> = parse(&bytes);
        assert!(matches!(result, Err(PeError::SectionDataOutOfBounds)));
    }

    #[test]
    fn rejects_write_and_execute_section() {
        let mut bytes = well_formed_image();
        let section_table_offset = DOS_HEADER_LEN + 4 + COFF_HEADER_LEN + 128;
        let characteristics_off = section_table_offset + 36;
        let wx = IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE | IMAGE_SCN_MEM_EXECUTE;
        bytes[characteristics_off..characteristics_off + 4].copy_from_slice(&wx.to_le_bytes());
        let result: Result<LoadDescriptor<4, 4>, PeError> = parse(&bytes);
        assert!(matches!(result, Err(PeError::WriteExecuteSection)));
    }

    #[test]
    fn rejects_more_sections_than_capacity() {
        let bytes = well_formed_image();
        let result: Result<LoadDescriptor<0, 4>, PeError> = parse(&bytes);
        assert!(matches!(result, Err(PeError::TooManySections)));
    }

    #[test]
    fn rejects_more_imports_than_capacity() {
        let bytes = well_formed_image();
        let result: Result<LoadDescriptor<4, 0>, PeError> = parse(&bytes);
        assert!(matches!(result, Err(PeError::TooManyImports)));
    }

    #[test]
    fn read_c_string_rejects_name_exceeding_capacity() {
        let bytes = b"MySymbol\0".to_vec();
        let result: Result<FixedBytes<4>, PeError> = read_c_string(&bytes, 0);
        assert_eq!(result, Err(PeError::NameTooLong));
    }

    #[test]
    fn read_c_string_rejects_missing_terminator() {
        let bytes = b"NoNul".to_vec();
        let result: Result<FixedBytes<64>, PeError> = read_c_string(&bytes, 0);
        assert_eq!(result, Err(PeError::NameOutOfBounds));
    }

    /// No truncation length of a well-formed image causes a panic; every
    /// prefix either fails closed with a typed [`PeError`] or (only at the
    /// full length) parses successfully. A lightweight stand-in for
    /// `cargo-fuzz` (not yet set up in this repo) over the truncation
    /// dimension specifically, per `agent/CODING_STANDARDS.md`'s
    /// adversarial-input-parser requirement.
    #[test]
    fn no_truncation_length_panics() {
        let bytes = well_formed_image();
        for len in 0..bytes.len() {
            let result: Result<LoadDescriptor<4, 4>, PeError> = parse(&bytes[..len]);
            assert!(result.is_err(), "prefix of length {len} unexpectedly parsed");
        }
        let full: Result<LoadDescriptor<4, 4>, PeError> = parse(&bytes);
        assert!(full.is_ok());
    }
}
