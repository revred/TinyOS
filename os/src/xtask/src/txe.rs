//! `pack`: a deterministic PE64 → TXE re-layout (`STORY-P0-08-01`).
//!
//! A TXE is not a new file format — it is a real PE32+ image, byte-for-byte
//! parseable by `exec::pe::parse` exactly as before, with two properties no
//! real-world linker's default output guarantees: every section's on-disk
//! `PointerToRawData` is [`PAGE_SIZE`]-aligned, and every section's
//! `SizeOfRawData` equals its `VirtualSize` (any `.bss`-style demand-zero
//! tail is physically zero-written into the file, not left implicit).
//! `exec::address_space::AddressSpace::create` (`STORY-P0-05-02`,
//! generalized in `STORY-P0-05-04`) already handles arbitrary file
//! alignment and `virtual_size > file_size` sections via a copy-based
//! mapper — `pack` exists anyway because x86-64 page tables can only ever
//! map whole aligned physical pages, so *some* copy is unavoidable for any
//! real linker's default `FileAlignment` (512), and doing that copy once,
//! deterministically, at build time (rather than on every boot) is the
//! actually elegant fix, not a workaround. Every image byte a caller cares
//! about — code, data, the import table (referenced by RVA, never a raw
//! file offset, so relocating section *file* positions never invalidates
//! it) — round-trips unchanged; only the on-disk section layout changes.
//!
//! This is host-side, `std` tooling (`xtask`'s own classification, per
//! `docs/mvp-delivery-strategy.md#crate-map`) — it runs once, offline, as
//! part of preparing a fixture or a deployment image, never on the running
//! kernel.

use std::vec::Vec;

const PAGE_SIZE: usize = 4096;

/// Errors [`pack`] fails closed with rather than reading past a
/// caller-declared boundary or panicking on malformed input — the same
/// discipline `exec::pe::parse` already applies to PE bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxePackError {
    /// The input is too short to contain a DOS header.
    Truncated,
    /// The DOS header's `"MZ"` signature is missing.
    InvalidDosSignature,
    /// The DOS header's `e_lfanew` field points outside the input.
    InvalidPeHeaderOffset,
    /// The `"PE\0\0"` signature at `e_lfanew` is missing.
    InvalidPeSignature,
    /// The optional header's magic is not `IMAGE_NT_OPTIONAL_HDR64_MAGIC`
    /// (`0x20b`) — this tool only re-layouts PE32+ images, matching
    /// `exec::pe::parse`'s own PE32+-only scope.
    NotPe32Plus,
    /// The section table runs past the end of the input.
    SectionTableOutOfBounds,
    /// A section's declared file offset and size run past the end of the
    /// input.
    SectionDataOutOfBounds,
}

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    bytes.get(offset..offset + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes.get(offset..offset + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn round_up_page(len: usize) -> usize {
    len.div_ceil(PAGE_SIZE) * PAGE_SIZE
}

/// Re-layouts `input` (a real PE32+ image) so every section's raw data
/// starts on a page boundary and fully covers its own `VirtualSize` —
/// zero-padded in the file itself, never left as an implicit demand-zero
/// gap. The DOS/PE/COFF/optional headers and the section table are copied
/// verbatim except for each section header's own `PointerToRawData`/
/// `SizeOfRawData` fields, patched in place to the new layout — every RVA
/// (virtual address), including the import directory's, is untouched, so
/// the output still parses via `exec::pe::parse` with identical section
/// permissions, identical imports, identical entry point.
pub fn pack(input: &[u8]) -> Result<Vec<u8>, TxePackError> {
    if input.len() < 0x40 {
        return Err(TxePackError::Truncated);
    }
    if &input[0..2] != b"MZ" {
        return Err(TxePackError::InvalidDosSignature);
    }
    let e_lfanew = u32_at(input, 0x3c).ok_or(TxePackError::Truncated)? as usize;
    let sig = input.get(e_lfanew..e_lfanew + 4).ok_or(TxePackError::InvalidPeHeaderOffset)?;
    if sig != b"PE\0\0" {
        return Err(TxePackError::InvalidPeSignature);
    }

    let coff = e_lfanew + 4;
    let num_sections = u16_at(input, coff + 2).ok_or(TxePackError::Truncated)? as usize;
    let opt_header_size = u16_at(input, coff + 16).ok_or(TxePackError::Truncated)? as usize;
    let opt_header_off = coff + 20;
    let magic = u16_at(input, opt_header_off).ok_or(TxePackError::Truncated)?;
    if magic != 0x20b {
        return Err(TxePackError::NotPe32Plus);
    }

    let section_table_off = opt_header_off + opt_header_size;
    let section_table_len = num_sections * 40;
    let section_table = input
        .get(section_table_off..section_table_off + section_table_len)
        .ok_or(TxePackError::SectionTableOutOfBounds)?
        .to_vec();

    // Everything up to (and including) the section table is copied
    // unchanged, then padded to a page boundary — the point after which
    // every section's new `PointerToRawData` starts.
    let headers_end = section_table_off + section_table_len;
    let headers = input.get(..headers_end).ok_or(TxePackError::Truncated)?;
    let mut output = headers.to_vec();
    output.resize(round_up_page(headers.len()), 0);

    let mut new_section_table = section_table.clone();
    for i in 0..num_sections {
        let entry = i * 40;
        let virtual_size = u32_at(&section_table, entry + 8).ok_or(TxePackError::Truncated)?;
        let old_raw_size = u32_at(&section_table, entry + 16).ok_or(TxePackError::Truncated)?;
        let old_raw_ptr = u32_at(&section_table, entry + 20).ok_or(TxePackError::Truncated)?;

        let copy_len = (old_raw_size as usize).min(virtual_size as usize);
        let src = input
            .get(old_raw_ptr as usize..old_raw_ptr as usize + copy_len)
            .ok_or(TxePackError::SectionDataOutOfBounds)?;

        let new_ptr = output.len() as u32;
        let new_size = round_up_page(virtual_size as usize);
        output.extend_from_slice(src);
        output.resize(output.len() + (new_size - copy_len), 0);

        new_section_table[entry + 16..entry + 20].copy_from_slice(&virtual_size.to_le_bytes());
        new_section_table[entry + 20..entry + 24].copy_from_slice(&new_ptr.to_le_bytes());
    }
    output[section_table_off..headers_end].copy_from_slice(&new_section_table);

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal, valid PE32+ image with `sections`, each given as
    /// `(virtual_size, file_bytes)` — `file_bytes.len()` becomes
    /// `SizeOfRawData`, deliberately *not* page-aligned and, for a `.bss`-
    /// style section, shorter than `virtual_size` — mirroring
    /// `exec::pe::tests::well_formed_image`'s own hand-built-PE approach,
    /// but with intentionally misaligned, non-page-multiple raw offsets
    /// (mirroring a real linker's default 512-byte `FileAlignment`), since
    /// that misalignment is exactly what `pack` exists to fix.
    fn synthetic_pe(sections: &[(u32, &[u8])]) -> Vec<u8> {
        let mut file = vec![0u8; 0x40];
        file[0] = b'M';
        file[1] = b'Z';
        let e_lfanew = 0x40u32;
        file[0x3c..0x40].copy_from_slice(&e_lfanew.to_le_bytes());

        file.extend_from_slice(b"PE\0\0");
        // COFF header: Machine, NumberOfSections, TimeDateStamp,
        // PointerToSymbolTable, NumberOfSymbols, SizeOfOptionalHeader,
        // Characteristics.
        file.extend_from_slice(&0x8664u16.to_le_bytes()); // Machine (AMD64)
        file.extend_from_slice(&(sections.len() as u16).to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes()); // TimeDateStamp
        file.extend_from_slice(&0u32.to_le_bytes()); // PointerToSymbolTable
        file.extend_from_slice(&0u32.to_le_bytes()); // NumberOfSymbols
        let opt_header_size = 112u16; // no data directories, minimal PE32+ optional header
        file.extend_from_slice(&opt_header_size.to_le_bytes());
        file.extend_from_slice(&0u16.to_le_bytes()); // Characteristics

        // Minimal PE32+ optional header: just the magic, padded to
        // `opt_header_size` — `pack` never reads past the magic field.
        let mut opt_header = vec![0u8; opt_header_size as usize];
        opt_header[0..2].copy_from_slice(&0x20bu16.to_le_bytes());
        file.extend_from_slice(&opt_header);

        // Section table: one 40-byte entry per section. Raw data is placed
        // immediately after the section table, back-to-back with no page
        // alignment at all (deliberately 512-byte-ish "linker-realistic"
        // spacing), so `pack` has real misalignment to fix.
        let section_table_off = file.len();
        file.resize(section_table_off + sections.len() * 40, 0);
        let mut raw_cursor = file.len();
        for (i, (virtual_size, bytes)) in sections.iter().enumerate() {
            let entry = section_table_off + i * 40;
            file[entry..entry + 8].copy_from_slice(b".sec\0\0\0\0");
            file[entry + 8..entry + 12].copy_from_slice(&virtual_size.to_le_bytes()); // VirtualSize
            file[entry + 12..entry + 16].copy_from_slice(&0u32.to_le_bytes()); // VirtualAddress (unused by pack)
            file[entry + 16..entry + 20].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
            file[entry + 20..entry + 24].copy_from_slice(&(raw_cursor as u32).to_le_bytes());

            file.resize(raw_cursor + bytes.len(), 0);
            file[raw_cursor..raw_cursor + bytes.len()].copy_from_slice(bytes);
            raw_cursor += bytes.len();
        }
        file
    }

    fn section_header(packed: &[u8], index: usize) -> &[u8] {
        let e_lfanew = u32_at(packed, 0x3c).unwrap() as usize;
        let coff = e_lfanew + 4;
        let num_sections = u16_at(packed, coff + 2).unwrap() as usize;
        let opt_header_size = u16_at(packed, coff + 16).unwrap() as usize;
        let section_table_off = coff + 20 + opt_header_size;
        assert!(index < num_sections);
        &packed[section_table_off + index * 40..section_table_off + (index + 1) * 40]
    }

    // STORY-P0-08-01 AC1: every section's new PointerToRawData is
    // page-aligned, even though the source image's wasn't.
    #[test]
    fn every_repacked_section_starts_on_a_page_boundary() {
        let input = synthetic_pe(&[(100, &[0xAA; 100]), (200, &[0xBB; 200])]);
        let packed = pack(&input).unwrap();
        for i in 0..2 {
            let header = section_header(&packed, i);
            let ptr = u32_at(header, 20).unwrap();
            assert_eq!(ptr as usize % PAGE_SIZE, 0);
        }
    }

    // STORY-P0-08-01 AC2: SizeOfRawData now equals VirtualSize exactly —
    // no implicit `.bss` gap left for the loader to special-case.
    #[test]
    fn every_repacked_sections_raw_size_equals_its_virtual_size() {
        let input = synthetic_pe(&[(2912, &[0xCC; 2560])]); // real .data shape: bss tail
        let packed = pack(&input).unwrap();
        let header = section_header(&packed, 0);
        let raw_size = u32_at(header, 16).unwrap();
        assert_eq!(raw_size, 2912);
    }

    // STORY-P0-08-01 AC3: real file-backed bytes are preserved exactly,
    // and the bss tail (and any page-rounding pad) is physically zeroed.
    #[test]
    fn section_content_is_preserved_and_the_tail_is_zeroed() {
        let input = synthetic_pe(&[(2912, &[0xCC; 2560])]);
        let packed = pack(&input).unwrap();
        let header = section_header(&packed, 0);
        let ptr = u32_at(header, 20).unwrap() as usize;
        let raw_size = u32_at(header, 16).unwrap() as usize;
        let data = &packed[ptr..ptr + raw_size];
        assert_eq!(&data[..2560], &[0xCC; 2560][..]);
        assert_eq!(&data[2560..2912], &[0u8; 352][..]);
    }

    // A non-page-multiple VirtualSize still gets a page-aligned *next*
    // section start — packed sections never overlap even when their own
    // sizes aren't page multiples.
    #[test]
    fn two_sections_with_non_page_aligned_sizes_never_overlap() {
        let input = synthetic_pe(&[(100, &[0xAA; 100]), (100, &[0xBB; 100])]);
        let packed = pack(&input).unwrap();
        let first = section_header(&packed, 0);
        let second = section_header(&packed, 1);
        let first_ptr = u32_at(first, 20).unwrap() as usize;
        let first_size = u32_at(first, 16).unwrap() as usize;
        let second_ptr = u32_at(second, 20).unwrap() as usize;
        assert!(first_ptr + first_size <= second_ptr);
    }

    #[test]
    fn rejects_input_missing_the_dos_signature() {
        let mut input = synthetic_pe(&[(100, &[0xAA; 100])]);
        input[0] = b'X';
        assert_eq!(pack(&input), Err(TxePackError::InvalidDosSignature));
    }

    #[test]
    fn rejects_truncated_input() {
        assert_eq!(pack(&[0u8; 4]), Err(TxePackError::Truncated));
    }
}
