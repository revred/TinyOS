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
    /// A section's declared virtual range overflows the 32-bit RVA space.
    SectionVirtualRangeOverflow,
    /// A section requests both write and execute permission (`G-PC-1`'s W^X
    /// requirement, enforced at parse time).
    WriteExecuteSection,
    /// More sections were found than the caller's `SECTIONS` capacity
    /// allows.
    TooManySections,
    /// An RVA (import directory, DLL name, or imported symbol name) does
    /// not fall inside any parsed section.
    RvaOutOfBounds,
    /// A PE64 import thunk sets bits that are reserved for its selected
    /// name/ordinal representation.
    MalformedImportThunk,
    /// The image entry point is not contained by any mapped section.
    EntryPointOutOfBounds,
    /// The image entry point is not in an executable, non-writable section.
    EntryPointNotExecutable,
    /// Adding the entry-point RVA to the preferred image base overflows.
    EntryPointAddressOverflow,
    /// A DLL or symbol name has no NUL terminator within the file.
    NameOutOfBounds,
    /// A DLL or symbol name exceeds this module's fixed buffer capacity.
    NameTooLong,
    /// More imported (DLL, symbol) pairs were found than the caller's
    /// `IMPORTS` capacity allows.
    TooManyImports,
    /// The import directory's declared byte range ended before its required
    /// all-zero terminator descriptor.
    ImportDirectoryUnterminated,
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

/// What one import thunk identifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSymbol {
    /// A symbol named by an `IMAGE_IMPORT_BY_NAME` record.
    Name(FixedBytes<MAX_SYMBOL_NAME_LEN>),
    /// A PE import-by-ordinal. It remains explicit so policy can deny or
    /// resolve it; omitting it would leave an unmediated IAT cell.
    Ordinal(u16),
}

/// One imported (DLL, symbol-or-ordinal) dependency, per `STORY-P0-05-01`
/// acceptance criterion 3 — resolution against an allowlist is
/// `STORY-P0-05-03`'s job, not this module's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportEntry {
    /// The importing DLL's name, e.g. `b"KERNEL32.dll"`.
    pub dll_name: FixedBytes<MAX_DLL_NAME_LEN>,
    /// The imported name or ordinal.
    pub symbol: ImportSymbol,
    /// Relative virtual address of *this import's own slot in the Import
    /// Address Table* — the 8-byte cell the image's own code indirects
    /// through at every call site for this symbol (`STORY-P1-03-03`).
    ///
    /// Recorded from the descriptor's `FirstThunk` (the IAT), never from
    /// `OriginalFirstThunk` (the ILT), even though the loader reads *names*
    /// from the ILT when one is present: the ILT is the immutable
    /// description of what was imported, the IAT is the mutable table a
    /// loader is expected to overwrite. Writing a resolved address into the
    /// ILT would leave every call site still indirecting through an
    /// unpatched IAT.
    ///
    /// Slot index counts every thunk, named or ordinal.
    pub iat_slot_rva: u32,
}

/// A parsed, validated PE64 image: its entry point, image base, sections,
/// and complete import dependency surface.
///
/// `SECTIONS` and `IMPORTS` are caller-chosen capacity bounds (analogous to
/// `Pool<T, N>` in `kernel::mem` and `hal::topology::Topology<N>`) — a file
/// declaring more sections or imports than fit fails closed via
/// [`PeError::TooManySections`]/[`PeError::TooManyImports`] rather than
/// growing unbounded storage.
#[derive(Debug, Clone, Copy)]
pub struct LoadDescriptor<const SECTIONS: usize, const IMPORTS: usize> {
    entry_point_rva: u32,
    image_base: u64,
    entry_virtual_address: u64,
    sections: [Option<SectionDescriptor>; SECTIONS],
    section_count: usize,
    imports: [Option<ImportEntry>; IMPORTS],
    import_count: usize,
}

impl<const SECTIONS: usize, const IMPORTS: usize> LoadDescriptor<SECTIONS, IMPORTS> {
    /// Relative virtual address of the entry point, proven by [`parse`] to
    /// lie in an executable, non-writable section.
    pub const fn entry_point_rva(&self) -> u32 {
        self.entry_point_rva
    }

    /// The image's preferred base virtual address.
    pub const fn image_base(&self) -> u64 {
        self.image_base
    }

    /// Checked sum of [`Self::image_base`] and [`Self::entry_point_rva`].
    ///
    /// Keeping this value inside the descriptor prevents activation paths
    /// from accidentally reintroducing unchecked entry-address arithmetic.
    pub const fn entry_virtual_address(&self) -> u64 {
        self.entry_virtual_address
    }

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
/// name. These remain explicit [`ImportSymbol::Ordinal`] dependencies so
/// policy and IAT patching cannot accidentally omit them.
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
fn read_c_string_bounded<const N: usize>(
    bytes: &[u8],
    offset: usize,
    limit: usize,
) -> Result<FixedBytes<N>, PeError> {
    let mut buf = [0u8; N];
    let mut len = 0usize;
    let mut i = offset;
    loop {
        if i >= limit {
            return Err(PeError::NameOutOfBounds);
        }
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

#[cfg(test)]
fn read_c_string<const N: usize>(bytes: &[u8], offset: usize) -> Result<FixedBytes<N>, PeError> {
    read_c_string_bounded(bytes, offset, bytes.len())
}

/// Translates a relative virtual address into a file byte offset by
/// finding the section whose *file-backed* range contains it. Virtual
/// zero-fill beyond `file_size` must never be treated as parser input.
/// Returns both the translated offset and the end of that section's raw
/// bytes so strings and tables cannot bleed into the next file region.
fn rva_to_file_window(sections: &[Option<SectionDescriptor>], rva: u32) -> Option<(usize, usize)> {
    for section in sections.iter().filter_map(Option::as_ref) {
        let end = section.virtual_address.checked_add(section.file_size)?;
        if rva >= section.virtual_address && rva < end {
            let delta = rva - section.virtual_address;
            let offset = (section.file_offset as usize).checked_add(delta as usize)?;
            let raw_end = (section.file_offset as usize).checked_add(section.file_size as usize)?;
            return Some((offset, raw_end));
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
        virtual_address
            .checked_add(virtual_size.max(size_of_raw_data))
            .ok_or(PeError::SectionVirtualRangeOverflow)?;

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

    let parsed_sections = &sections[..section_count];
    let entry_section = parsed_sections
        .iter()
        .filter_map(Option::as_ref)
        .find(|section| {
            let extent = section.virtual_size.max(section.file_size);
            section.virtual_address.checked_add(extent).is_some_and(|end| {
                entry_point_rva >= section.virtual_address && entry_point_rva < end
            })
        })
        .ok_or(PeError::EntryPointOutOfBounds)?;
    if !entry_section.permissions.execute || entry_section.permissions.write {
        return Err(PeError::EntryPointNotExecutable);
    }
    let entry_virtual_address = image_base
        .checked_add(u64::from(entry_point_rva))
        .ok_or(PeError::EntryPointAddressOverflow)?;

    let mut imports: [Option<ImportEntry>; IMPORTS] = [None; IMPORTS];
    let mut import_count = 0usize;
    if import_dir_rva != 0 && import_dir_size != 0 {
        import_dir_rva.checked_add(import_dir_size).ok_or(PeError::RvaOutOfBounds)?;
        let mut descriptor_bytes = 0u32;
        loop {
            let next_descriptor_bytes = descriptor_bytes
                .checked_add(IMPORT_DESCRIPTOR_LEN as u32)
                .ok_or(PeError::RvaOutOfBounds)?;
            if next_descriptor_bytes > import_dir_size {
                return Err(PeError::ImportDirectoryUnterminated);
            }
            let desc_rva =
                import_dir_rva.checked_add(descriptor_bytes).ok_or(PeError::RvaOutOfBounds)?;
            let (desc_offset, desc_raw_end) =
                rva_to_file_window(parsed_sections, desc_rva).ok_or(PeError::RvaOutOfBounds)?;
            let entry_end =
                desc_offset.checked_add(IMPORT_DESCRIPTOR_LEN).ok_or(PeError::Truncated)?;
            if entry_end > desc_raw_end || entry_end > bytes.len() {
                return Err(PeError::Truncated);
            }
            let original_first_thunk = read_u32(bytes, desc_offset)?;
            let name_rva = read_u32(bytes, desc_offset + 12)?;
            let first_thunk = read_u32(bytes, desc_offset + 16)?;
            if original_first_thunk == 0 && name_rva == 0 && first_thunk == 0 {
                break;
            }

            let (dll_name_offset, dll_name_limit) =
                rva_to_file_window(parsed_sections, name_rva).ok_or(PeError::RvaOutOfBounds)?;
            let dll_name: FixedBytes<MAX_DLL_NAME_LEN> =
                read_c_string_bounded(bytes, dll_name_offset, dll_name_limit)?;

            // Names are read from the ILT when the image has one (it is the
            // immutable copy); IAT slot addresses always come from
            // `FirstThunk`, since that is the table a loader patches. When
            // an image has no separate ILT the two coincide.
            let thunk_rva =
                if original_first_thunk != 0 { original_first_thunk } else { first_thunk };
            let iat_base_rva = if first_thunk != 0 { first_thunk } else { thunk_rva };
            if thunk_rva != 0 {
                let mut thunk_index = 0u32;
                loop {
                    let thunk_delta =
                        thunk_index.checked_mul(THUNK_LEN as u32).ok_or(PeError::RvaOutOfBounds)?;
                    let current_thunk_rva =
                        thunk_rva.checked_add(thunk_delta).ok_or(PeError::RvaOutOfBounds)?;
                    let (thunk_offset, thunk_raw_end) =
                        rva_to_file_window(parsed_sections, current_thunk_rva)
                            .ok_or(PeError::RvaOutOfBounds)?;
                    let thunk_end =
                        thunk_offset.checked_add(THUNK_LEN).ok_or(PeError::Truncated)?;
                    if thunk_end > thunk_raw_end || thunk_end > bytes.len() {
                        return Err(PeError::Truncated);
                    }
                    let thunk = read_u64(bytes, thunk_offset)?;
                    if thunk == 0 {
                        break;
                    }
                    let symbol = if thunk & IMAGE_ORDINAL_FLAG64 != 0 {
                        if thunk & !(IMAGE_ORDINAL_FLAG64 | 0xFFFF) != 0 {
                            return Err(PeError::MalformedImportThunk);
                        }
                        ImportSymbol::Ordinal((thunk & 0xFFFF) as u16)
                    } else {
                        if thunk > u64::from(u32::MAX) {
                            return Err(PeError::MalformedImportThunk);
                        }
                        let hint_name_rva = thunk as u32;
                        let (hint_name_offset, hint_name_limit) =
                            rva_to_file_window(parsed_sections, hint_name_rva)
                                .ok_or(PeError::RvaOutOfBounds)?;
                        // Skip the 2-byte `Hint` field preceding the name.
                        let symbol_offset =
                            hint_name_offset.checked_add(2).ok_or(PeError::RvaOutOfBounds)?;
                        if symbol_offset > hint_name_limit {
                            return Err(PeError::RvaOutOfBounds);
                        }
                        let name: FixedBytes<MAX_SYMBOL_NAME_LEN> =
                            read_c_string_bounded(bytes, symbol_offset, hint_name_limit)?;
                        ImportSymbol::Name(name)
                    };
                    if import_count >= IMPORTS {
                        return Err(PeError::TooManyImports);
                    }
                    let iat_slot_rva =
                        iat_base_rva.checked_add(thunk_delta).ok_or(PeError::RvaOutOfBounds)?;
                    imports[import_count] = Some(ImportEntry { dll_name, symbol, iat_slot_rva });
                    import_count += 1;
                    thunk_index = thunk_index.checked_add(1).ok_or(PeError::RvaOutOfBounds)?;
                }
            }
            descriptor_bytes = next_descriptor_bytes;
        }
    }

    Ok(LoadDescriptor {
        entry_point_rva,
        image_base,
        entry_virtual_address,
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
        image_around(import_block)
    }

    /// Wraps an already-built import block in the DOS/PE/COFF/optional
    /// headers and the single `.text` section that carries it — shared by
    /// [`well_formed_image`] and [`image_with_split_ilt_and_iat`] so the two
    /// differ only in the import layout under test.
    fn image_around(import_block: std::vec::Vec<u8>) -> std::vec::Vec<u8> {
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
        assert_eq!(descriptor.entry_point_rva(), IMAGE_BASE_RVA + 4);
        assert_eq!(descriptor.image_base(), 0x1_4000_0000);
        assert_eq!(
            descriptor.entry_virtual_address(),
            0x1_4000_0000 + u64::from(IMAGE_BASE_RVA + 4)
        );

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
        assert!(matches!(
            imports[0].symbol,
            ImportSymbol::Name(name) if name.as_bytes() == b"MySymbol"
        ));
        // STORY-P1-03-03: the IAT cell this import's call sites indirect
        // through — `FirstThunk` plus this import's slot index (0 here).
        assert_eq!(imports[0].iat_slot_rva, IMAGE_BASE_RVA + 40);
    }

    /// The same shape as [`well_formed_image`], but with a *separate* ILT
    /// and IAT (the layout every real linker emits), so a parser that
    /// recorded the ILT address would compute the wrong cell for `MySymbol`.
    fn image_with_split_ilt_and_iat() -> std::vec::Vec<u8> {
        let ilt_rva = IMAGE_BASE_RVA + 40;
        let iat_rva = IMAGE_BASE_RVA + 56;
        let iibn_rva = IMAGE_BASE_RVA + 72;
        let name_rva = IMAGE_BASE_RVA + 83;

        let mut import_block = std::vec::Vec::new();
        import_block.extend_from_slice(&ilt_rva.to_le_bytes()); // OriginalFirstThunk
        import_block.extend_from_slice(&0u32.to_le_bytes());
        import_block.extend_from_slice(&0u32.to_le_bytes());
        import_block.extend_from_slice(&name_rva.to_le_bytes()); // Name
        import_block.extend_from_slice(&iat_rva.to_le_bytes()); // FirstThunk
        import_block.extend_from_slice(&[0u8; 20]); // null descriptor
        assert_eq!(import_block.len(), 40);

        // ILT: one named import, then terminator.
        import_block.extend_from_slice(&(iibn_rva as u64).to_le_bytes());
        import_block.extend_from_slice(&0u64.to_le_bytes());
        assert_eq!(import_block.len(), 56);
        // IAT: same two cells, holding the unpatched thunk values.
        import_block.extend_from_slice(&(iibn_rva as u64).to_le_bytes());
        import_block.extend_from_slice(&0u64.to_le_bytes());
        assert_eq!(import_block.len(), 72);
        import_block.extend_from_slice(&0u16.to_le_bytes()); // Hint
        import_block.extend_from_slice(b"MySymbol\0");
        assert_eq!(import_block.len(), 83);
        import_block.extend_from_slice(b"MYDLL.DLL\0");

        image_around(import_block)
    }

    // STORY-P1-03-03: the slot address comes from `FirstThunk`, not the ILT.
    // The mistake is silent — it patches a real, wrong cell — so it is
    // pinned here.
    #[test]
    fn the_iat_slot_comes_from_first_thunk() {
        let bytes = image_with_split_ilt_and_iat();
        let descriptor: LoadDescriptor<4, 4> = parse(&bytes).unwrap();
        let imports: std::vec::Vec<_> = descriptor.imports().collect();
        assert_eq!(imports.len(), 1);
        assert!(matches!(
            imports[0].symbol,
            ImportSymbol::Name(name) if name.as_bytes() == b"MySymbol"
        ));
        assert_eq!(imports[0].iat_slot_rva, IMAGE_BASE_RVA + 56);
        assert_ne!(
            imports[0].iat_slot_rva,
            IMAGE_BASE_RVA + 40,
            "the ILT address must never be recorded as the patch target"
        );
    }

    #[test]
    fn models_ordinal_imports_instead_of_omitting_them_from_policy() {
        let mut bytes = well_formed_image();
        let raw_data_offset = DOS_HEADER_LEN + 4 + COFF_HEADER_LEN + 128 + SECTION_HEADER_LEN;
        bytes[raw_data_offset + 40..raw_data_offset + 48]
            .copy_from_slice(&(IMAGE_ORDINAL_FLAG64 | 7).to_le_bytes());
        let descriptor: LoadDescriptor<4, 4> = parse(&bytes).unwrap();
        let imports: std::vec::Vec<_> = descriptor.imports().collect();
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].symbol, ImportSymbol::Ordinal(7));
        assert_eq!(imports[0].iat_slot_rva, IMAGE_BASE_RVA + 40);
    }

    #[test]
    fn rejects_reserved_bits_in_named_and_ordinal_thunks() {
        let raw_data_offset = DOS_HEADER_LEN + 4 + COFF_HEADER_LEN + 128 + SECTION_HEADER_LEN;

        let mut named = well_formed_image();
        named[raw_data_offset + 40..raw_data_offset + 48]
            .copy_from_slice(&0x0000_0001_0000_1000u64.to_le_bytes());
        let named_result: Result<LoadDescriptor<4, 4>, PeError> = parse(&named);
        assert_eq!(named_result.unwrap_err(), PeError::MalformedImportThunk);

        let mut ordinal = well_formed_image();
        ordinal[raw_data_offset + 40..raw_data_offset + 48]
            .copy_from_slice(&(IMAGE_ORDINAL_FLAG64 | (1 << 16) | 7).to_le_bytes());
        let ordinal_result: Result<LoadDescriptor<4, 4>, PeError> = parse(&ordinal);
        assert_eq!(ordinal_result.unwrap_err(), PeError::MalformedImportThunk);
    }

    #[test]
    fn parsing_is_deterministic() {
        let bytes = well_formed_image();
        let first: LoadDescriptor<4, 4> = parse(&bytes).unwrap();
        let second: LoadDescriptor<4, 4> = parse(&bytes).unwrap();
        assert_eq!(first.entry_point_rva(), second.entry_point_rva());
        assert_eq!(first.image_base(), second.image_base());
        assert_eq!(first.entry_virtual_address(), second.entry_virtual_address());
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
    fn rejects_entry_point_outside_every_section() {
        let mut bytes = well_formed_image();
        let opt_off = DOS_HEADER_LEN + 4 + COFF_HEADER_LEN;
        bytes[opt_off + 16..opt_off + 20].copy_from_slice(&0x2000u32.to_le_bytes());
        let result: Result<LoadDescriptor<4, 4>, PeError> = parse(&bytes);
        assert_eq!(result.unwrap_err(), PeError::EntryPointOutOfBounds);
    }

    #[test]
    fn rejects_entry_point_in_non_executable_section() {
        let mut bytes = well_formed_image();
        let section_table_offset = DOS_HEADER_LEN + 4 + COFF_HEADER_LEN + 128;
        let characteristics_off = section_table_offset + 36;
        bytes[characteristics_off..characteristics_off + 4]
            .copy_from_slice(&(IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE).to_le_bytes());
        let result: Result<LoadDescriptor<4, 4>, PeError> = parse(&bytes);
        assert_eq!(result.unwrap_err(), PeError::EntryPointNotExecutable);
    }

    #[test]
    fn rejects_entry_point_address_addition_overflow() {
        let mut bytes = well_formed_image();
        let opt_off = DOS_HEADER_LEN + 4 + COFF_HEADER_LEN;
        let overflowing_base = u64::MAX - u64::from(IMAGE_BASE_RVA);
        bytes[opt_off + 24..opt_off + 32].copy_from_slice(&overflowing_base.to_le_bytes());
        let result: Result<LoadDescriptor<4, 4>, PeError> = parse(&bytes);
        assert_eq!(result.unwrap_err(), PeError::EntryPointAddressOverflow);
    }

    #[test]
    fn rejects_import_directory_without_terminator_inside_declared_size() {
        let mut bytes = well_formed_image();
        let opt_off = DOS_HEADER_LEN + 4 + COFF_HEADER_LEN;
        let dd1_off = opt_off + IMPORT_DIRECTORY_ENTRY_OFFSET;
        bytes[dd1_off + 4..dd1_off + 8]
            .copy_from_slice(&(IMPORT_DESCRIPTOR_LEN as u32).to_le_bytes());
        let result: Result<LoadDescriptor<4, 4>, PeError> = parse(&bytes);
        assert_eq!(result.unwrap_err(), PeError::ImportDirectoryUnterminated);
    }

    #[test]
    fn rejects_import_rva_in_virtual_zero_fill() {
        let mut bytes = well_formed_image();
        let section_table_offset = DOS_HEADER_LEN + 4 + COFF_HEADER_LEN + 128;
        bytes[section_table_offset + 16..section_table_offset + 20]
            .copy_from_slice(&40u32.to_le_bytes());
        let result: Result<LoadDescriptor<4, 4>, PeError> = parse(&bytes);
        assert_eq!(result.unwrap_err(), PeError::RvaOutOfBounds);
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
