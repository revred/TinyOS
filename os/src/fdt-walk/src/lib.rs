//! One fact from a hostile tree, or a named refusal (`STORY-P1-13-01`).
//!
//! A flattened device tree is a **variable-length device format** — the class
//! the C1 input rule forbids by name, `PD-12` keeps outside C1, and `BND-03`
//! counts to zero in privileged linked bytes. This crate exists so that the
//! parse can exist *without* entering the image: it is a pure leaf crate,
//! linked into nothing the board boots (asserted by test against the image
//! crates' own manifests), whose consumers are a future C4 domain and — nearer
//! — the host as an off-board disposable inspector of a DTB the boot shipped
//! aside as data. The containment decision is recorded in the Story; this
//! crate is the shape-independent half.
//!
//! # Discipline
//!
//! - **Total.** Every byte string maps to exactly one `Ok` or one *named*
//!   [`Refusal`], in bounded work. No panic path, no partial answer.
//! - **Bounded.** Every read is bounds-checked against a capped `totalsize`;
//!   nesting is capped at [`MAX_DEPTH`]; the token walk is capped at
//!   [`MAX_TOKENS`]. The caps are compile-time constants pinned by test, so a
//!   widening is a reviewed diff, never drift.
//! - **Allocation-free and pure.** A function of the byte slice, `no_std`,
//!   `forbid(unsafe_code)`, no state left behind.
//! - **One fact.** The `reg` of the first completed node whose `compatible`
//!   list contains exactly `simple-framebuffer`. The moment a second consumer
//!   wants a second node, that is a new Story with its own adversarial
//!   evidence (`FEAT-P1-13` non-goals), not a generalisation here.
//! - **Output is data, never authority.** The extracted address justifies a
//!   write target only after the consumer's own MMU map and size bounds verify
//!   it; nothing here maps, writes, or trusts.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Hard cap on the `totalsize` a header may claim, in bytes.
///
/// The Pi 5 firmware's blob is ~50 KiB; 256 KiB is generous headroom without
/// letting a lying header size a quarter-gigabyte walk. A cap, not a
/// measurement — stated so the margin is reviewable.
pub const MAX_TOTALSIZE: u32 = 256 * 1024;

/// Hard cap on node nesting depth. Real firmware trees run ~6 deep.
pub const MAX_DEPTH: usize = 16;

/// Hard cap on tokens walked. A 50 KiB blob holds ~12,800 tokens; this cap is
/// reachable *within* [`MAX_TOTALSIZE`] (32,769 NOPs fit in 129 KiB), so the
/// refusal it names is testable rather than theoretical.
pub const MAX_TOKENS: u32 = 32_768;

const HEADER_LEN: usize = 40;
const FDT_MAGIC: u32 = 0xd00d_feed;
const MIN_VERSION: u32 = 17;

const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_NOP: u32 = 4;
const FDT_END: u32 = 9;

/// The extracted fact: the `simple-framebuffer` node's first `(address, size)`
/// pair, untranslated (see [`Refusal::RangesUnsupported`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FbReg {
    /// The address cells of the first `reg` entry, big-endian-folded.
    pub base: u64,
    /// The size cells of the first `reg` entry, big-endian-folded.
    pub size: u64,
}

/// Every way the walk declines, each distinct — a refusal that cannot name
/// itself is a refusal an operator diagnoses at the cost of a bench session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Fewer bytes than the 40-byte header.
    TooShortForHeader,
    /// The magic word is not `0xd00dfeed`.
    BadMagic,
    /// Header version below 17 — earlier layouts differ and are not walked.
    VersionUnsupported,
    /// `totalsize` claims more than [`MAX_TOTALSIZE`].
    TotalSizeOverCap,
    /// `totalsize` claims more bytes than were presented, or fewer than the
    /// header itself — the one field every later bound derives from, refused
    /// before any region is read.
    TotalSizeLying,
    /// The structure region is misaligned, inside the header, or out of
    /// bounds.
    StructRegionInvalid,
    /// The strings region is inside the header or out of bounds.
    StringsRegionInvalid,
    /// The structure and strings regions overlap — self-referential layout.
    RegionsOverlap,
    /// Nodes nested deeper than [`MAX_DEPTH`].
    DepthOverCap,
    /// More tokens than [`MAX_TOKENS`].
    TokenOverCap,
    /// A token value the format does not define.
    TokenUnknown,
    /// The structure region ended in the middle of a token, or without
    /// reaching `FDT_END`.
    StructTruncated,
    /// A node name with no terminator before the region ends.
    NameUnterminated,
    /// A property's declared length runs past the structure region.
    PropOutOfBounds,
    /// A property name offset outside the strings region, or a name with no
    /// terminator inside it.
    PropNameInvalid,
    /// `END_NODE` with no node open, `END` with nodes still open, or a
    /// property outside any node.
    StructureMalformed,
    /// No node's `compatible` list contains exactly `simple-framebuffer`.
    NodeAbsent,
    /// The matched node has no `reg` property.
    RegAbsent,
    /// The matched node's `reg` is too short for one (address, size) pair.
    RegMalformed,
    /// `#address-cells`/`#size-cells` outside 1..=2, or the property is not
    /// four bytes — folding wider cells into `u64` would truncate silently.
    CellsUnsupported,
    /// An ancestor carries a non-empty `ranges`: the answer would need
    /// bus-address translation this increment does not implement. The honest
    /// answer until the target blob demonstrates the need (`TEST-P1-13-01-A`
    /// clause 6).
    RangesUnsupported,
}

/// What one open node has declared so far, tracked per depth.
#[derive(Clone, Copy)]
struct NodeFrame {
    /// `#address-cells` for this node's children (spec default 2).
    addr_cells: u32,
    /// `#size-cells` for this node's children (spec default 1).
    size_cells: u32,
    /// Whether this node carries a non-empty `ranges`.
    ranges_nonempty: bool,
    /// Whether this node's `compatible` list matched.
    matched: bool,
    /// Absolute offset and length of this node's `reg` value, if seen.
    reg: Option<(usize, usize)>,
}

const EMPTY_FRAME: NodeFrame =
    NodeFrame { addr_cells: 2, size_cells: 1, ranges_nonempty: false, matched: false, reg: None };

/// Extracts the `simple-framebuffer` node's `reg`, or refuses by name.
///
/// Pure over the slice; see the crate docs for the discipline. The first node
/// whose definition **completes** with a matching `compatible` wins,
/// deterministically. Bytes beyond the header's own (validated) `totalsize`
/// are ignored.
///
/// # Errors
///
/// Every malformed, over-cap, lying, or absent condition is a distinct
/// [`Refusal`]; see each variant.
pub fn simple_framebuffer_reg(blob: &[u8]) -> Result<FbReg, Refusal> {
    // ---- clause 1: the header, believed only within its own caps ----------
    if blob.len() < HEADER_LEN {
        return Err(Refusal::TooShortForHeader);
    }
    if be32(blob, 0) != FDT_MAGIC {
        return Err(Refusal::BadMagic);
    }
    if be32(blob, 20) < MIN_VERSION {
        return Err(Refusal::VersionUnsupported);
    }
    let totalsize = be32(blob, 4);
    if totalsize > MAX_TOTALSIZE {
        return Err(Refusal::TotalSizeOverCap);
    }
    let totalsize = totalsize as usize;
    if totalsize > blob.len() || totalsize < HEADER_LEN {
        return Err(Refusal::TotalSizeLying);
    }
    // Everything after this line reads only inside the validated claim.
    let blob = &blob[..totalsize];

    // ---- clause 2: regions validated before they are walked ---------------
    let off_struct = be32(blob, 8) as usize;
    let size_struct = be32(blob, 36) as usize;
    let off_strings = be32(blob, 12) as usize;
    let size_strings = be32(blob, 32) as usize;
    if !off_struct.is_multiple_of(4)
        || off_struct < HEADER_LEN
        || off_struct.checked_add(size_struct).is_none_or(|end| end > totalsize)
    {
        return Err(Refusal::StructRegionInvalid);
    }
    if off_strings < HEADER_LEN
        || off_strings.checked_add(size_strings).is_none_or(|end| end > totalsize)
    {
        return Err(Refusal::StringsRegionInvalid);
    }
    if size_struct > 0
        && size_strings > 0
        && off_struct < off_strings + size_strings
        && off_strings < off_struct + size_struct
    {
        return Err(Refusal::RegionsOverlap);
    }
    let strings = &blob[off_strings..off_strings + size_strings];

    // ---- clauses 3-6: the bounded walk ------------------------------------
    let region = &blob[off_struct..off_struct + size_struct];
    let mut cursor: usize = 0;
    let mut tokens: u32 = 0;
    // Depth 0 is the virtual parent of the root, carrying the spec defaults
    // (2 address cells, 1 size cell) so the root's own `reg` — degenerate but
    // expressible — reads under defined semantics.
    let mut stack = [EMPTY_FRAME; MAX_DEPTH + 1];
    let mut depth: usize = 0;

    loop {
        if cursor + 4 > region.len() {
            return Err(Refusal::StructTruncated);
        }
        if tokens >= MAX_TOKENS {
            return Err(Refusal::TokenOverCap);
        }
        tokens += 1;
        let token = be32(region, cursor);
        cursor += 4;

        match token {
            FDT_BEGIN_NODE => {
                if depth >= MAX_DEPTH {
                    return Err(Refusal::DepthOverCap);
                }
                // The name: a NUL-terminated string inside the region. Its
                // content is never interpreted — matching is by `compatible`.
                let Some(nul) = region[cursor..].iter().position(|byte| *byte == 0) else {
                    return Err(Refusal::NameUnterminated);
                };
                cursor += nul + 1;
                cursor = cursor.div_ceil(4) * 4;
                depth += 1;
                stack[depth] = EMPTY_FRAME;
            }
            FDT_END_NODE => {
                if depth == 0 {
                    return Err(Refusal::StructureMalformed);
                }
                let frame = stack[depth];
                depth -= 1;
                if frame.matched {
                    // First completed match wins; a matched node that cannot
                    // be read is hostile input, refused rather than skipped.
                    return evaluate(blob, &frame, &stack[..=depth]);
                }
            }
            FDT_PROP => {
                if depth == 0 {
                    return Err(Refusal::StructureMalformed);
                }
                if cursor + 8 > region.len() {
                    return Err(Refusal::StructTruncated);
                }
                let len = be32(region, cursor) as usize;
                let nameoff = be32(region, cursor + 4) as usize;
                cursor += 8;
                if cursor.checked_add(len).is_none_or(|end| end > region.len()) {
                    return Err(Refusal::PropOutOfBounds);
                }
                let value_at = off_struct + cursor;
                cursor += len;
                cursor = cursor.div_ceil(4) * 4;

                // The name resolves in the strings region, bounded there.
                if nameoff >= strings.len() {
                    return Err(Refusal::PropNameInvalid);
                }
                let Some(name_nul) = strings[nameoff..].iter().position(|byte| *byte == 0) else {
                    return Err(Refusal::PropNameInvalid);
                };
                let name = &strings[nameoff..nameoff + name_nul];
                let value = &blob[value_at..value_at + len];

                let frame = &mut stack[depth];
                match name {
                    b"#address-cells" => frame.addr_cells = cells_value(value)?,
                    b"#size-cells" => frame.size_cells = cells_value(value)?,
                    b"ranges" => frame.ranges_nonempty = !value.is_empty(),
                    b"compatible" => {
                        if value
                            .split(|byte| *byte == 0)
                            .any(|entry| entry == b"simple-framebuffer")
                        {
                            frame.matched = true;
                        }
                    }
                    b"reg" => frame.reg = Some((value_at, len)),
                    _ => {}
                }
            }
            FDT_NOP => {}
            FDT_END => {
                if depth != 0 {
                    return Err(Refusal::StructureMalformed);
                }
                return Err(Refusal::NodeAbsent);
            }
            _ => return Err(Refusal::TokenUnknown),
        }
    }
}

/// Reads one big-endian `u32`. The caller has already bounds-checked; this is
/// a helper, not a guard, and it stays private for that reason.
fn be32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// A `#address-cells`/`#size-cells` value: exactly four bytes, 1..=2.
///
/// Wider counts exist in the spec; folding them into a `u64` would truncate
/// silently, so they are refused until a real blob demonstrates the need.
fn cells_value(value: &[u8]) -> Result<u32, Refusal> {
    if value.len() != 4 {
        return Err(Refusal::CellsUnsupported);
    }
    let cells = be32(value, 0);
    if !(1..=2).contains(&cells) {
        return Err(Refusal::CellsUnsupported);
    }
    Ok(cells)
}

/// Evaluates a completed, matched node against its ancestors.
///
/// `ancestors` is the open stack up to and including the node's parent;
/// entry 0 is the virtual spec-defaults frame, entries 1.. are real open
/// nodes whose `ranges` would govern translation.
fn evaluate(blob: &[u8], frame: &NodeFrame, ancestors: &[NodeFrame]) -> Result<FbReg, Refusal> {
    // Clause 6: translation is refused, never guessed. Only real ancestor
    // nodes are consulted — the virtual defaults frame carries no `ranges`.
    if ancestors.iter().skip(1).any(|ancestor| ancestor.ranges_nonempty) {
        return Err(Refusal::RangesUnsupported);
    }
    let Some((reg_at, reg_len)) = frame.reg else {
        return Err(Refusal::RegAbsent);
    };
    let parent = ancestors[ancestors.len() - 1];
    // Cells were validated as they were parsed; the defaults are in range by
    // construction, so these reads cannot exceed 2.
    let addr_cells = parent.addr_cells as usize;
    let size_cells = parent.size_cells as usize;
    let pair_len = (addr_cells + size_cells) * 4;
    if reg_len < pair_len {
        return Err(Refusal::RegMalformed);
    }
    Ok(FbReg {
        base: fold_cells(blob, reg_at, addr_cells),
        size: fold_cells(blob, reg_at + addr_cells * 4, size_cells),
    })
}

/// Folds 1..=2 big-endian cells into a `u64`. Bounds hold by the caller's
/// `pair_len` check against the property's validated extent.
fn fold_cells(blob: &[u8], at: usize, cells: usize) -> u64 {
    let mut value: u64 = 0;
    for cell in 0..cells {
        value = (value << 32) | u64::from(be32(blob, at + cell * 4));
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- builder -----------------------------------------------------------

    /// Builds structurally valid blobs the tests then bend one field at a
    /// time. Layout: header (40) | rsvmap (16 zero bytes) | struct | strings.
    struct Blob {
        structure: Vec<u8>,
        strings: Vec<u8>,
    }

    impl Blob {
        fn new() -> Self {
            Blob { structure: Vec::new(), strings: Vec::new() }
        }

        fn token(&mut self, token: u32) {
            self.structure.extend_from_slice(&token.to_be_bytes());
        }

        fn begin(&mut self, name: &str) {
            self.token(FDT_BEGIN_NODE);
            self.structure.extend_from_slice(name.as_bytes());
            self.structure.push(0);
            while !self.structure.len().is_multiple_of(4) {
                self.structure.push(0);
            }
        }

        fn end(&mut self) {
            self.token(FDT_END_NODE);
        }

        fn string_off(&mut self, name: &str) -> u32 {
            // Dedup by scan; the strings block is NUL-delimited entries.
            let needle: Vec<u8> = name.bytes().chain([0]).collect();
            let mut at = 0;
            while at + needle.len() <= self.strings.len() {
                if self.strings[at..at + needle.len()] == needle[..] {
                    return u32::try_from(at).unwrap();
                }
                at += 1;
            }
            let off = u32::try_from(self.strings.len()).unwrap();
            self.strings.extend_from_slice(&needle);
            off
        }

        fn prop(&mut self, name: &str, value: &[u8]) {
            let nameoff = self.string_off(name);
            self.token(FDT_PROP);
            self.structure.extend_from_slice(&u32::try_from(value.len()).unwrap().to_be_bytes());
            self.structure.extend_from_slice(&nameoff.to_be_bytes());
            self.structure.extend_from_slice(value);
            while !self.structure.len().is_multiple_of(4) {
                self.structure.push(0);
            }
        }

        fn prop_cells(&mut self, name: &str, cells: &[u32]) {
            let mut value = Vec::new();
            for cell in cells {
                value.extend_from_slice(&cell.to_be_bytes());
            }
            self.prop(name, &value);
        }

        fn fdt_end(&mut self) {
            self.token(FDT_END);
        }

        fn build(&self) -> Vec<u8> {
            let off_struct = HEADER_LEN + 16;
            let off_strings = off_struct + self.structure.len();
            let totalsize = off_strings + self.strings.len();
            let mut blob = Vec::with_capacity(totalsize);
            blob.extend_from_slice(&FDT_MAGIC.to_be_bytes());
            blob.extend_from_slice(&u32::try_from(totalsize).unwrap().to_be_bytes());
            blob.extend_from_slice(&u32::try_from(off_struct).unwrap().to_be_bytes());
            blob.extend_from_slice(&u32::try_from(off_strings).unwrap().to_be_bytes());
            blob.extend_from_slice(&u32::try_from(HEADER_LEN).unwrap().to_be_bytes()); // rsvmap
            blob.extend_from_slice(&17u32.to_be_bytes()); // version
            blob.extend_from_slice(&16u32.to_be_bytes()); // last_comp_version
            blob.extend_from_slice(&0u32.to_be_bytes()); // boot_cpuid_phys
            blob.extend_from_slice(&u32::try_from(self.strings.len()).unwrap().to_be_bytes());
            blob.extend_from_slice(&u32::try_from(self.structure.len()).unwrap().to_be_bytes());
            blob.extend_from_slice(&[0u8; 16]); // one empty rsvmap entry
            blob.extend_from_slice(&self.structure);
            blob.extend_from_slice(&self.strings);
            blob
        }
    }

    fn set_be32(blob: &mut [u8], at: usize, value: u32) {
        blob[at..at + 4].copy_from_slice(&value.to_be_bytes());
    }

    /// A well-formed tree: root with defaults, one framebuffer child.
    fn framebuffer_blob() -> Vec<u8> {
        let mut b = Blob::new();
        b.begin("");
        b.begin("framebuffer@1eaa9000");
        b.prop("compatible", b"simple-framebuffer\0");
        // Root defaults: 2 address cells, 1 size cell.
        b.prop_cells("reg", &[0x0, 0x1eaa_9000, 0x007f_8000]);
        b.end();
        b.end();
        b.fdt_end();
        b.build()
    }

    // ---- clause 1: the header is believed only within its own caps --------

    #[test]
    fn shorter_than_the_header_is_refused() {
        assert_eq!(simple_framebuffer_reg(&[0u8; 39]), Err(Refusal::TooShortForHeader));
    }

    #[test]
    fn a_wrong_magic_is_refused() {
        let mut blob = framebuffer_blob();
        set_be32(&mut blob, 0, 0xdead_beef);
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::BadMagic));
    }

    #[test]
    fn a_version_below_seventeen_is_refused() {
        let mut blob = framebuffer_blob();
        set_be32(&mut blob, 20, 16);
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::VersionUnsupported));
    }

    #[test]
    fn a_totalsize_over_the_cap_is_refused_before_the_lying_check() {
        let mut blob = framebuffer_blob();
        set_be32(&mut blob, 4, MAX_TOTALSIZE + 4);
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::TotalSizeOverCap));
    }

    #[test]
    fn a_totalsize_beyond_the_presented_bytes_is_refused() {
        let mut blob = framebuffer_blob();
        let lying = u32::try_from(blob.len()).unwrap() + 4;
        set_be32(&mut blob, 4, lying);
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::TotalSizeLying));
    }

    #[test]
    fn a_totalsize_smaller_than_the_header_is_refused() {
        let mut blob = framebuffer_blob();
        set_be32(&mut blob, 4, 8);
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::TotalSizeLying));
    }

    // ---- clause 2: regions validated before they are walked ---------------

    #[test]
    fn a_misaligned_struct_region_is_refused() {
        let mut blob = framebuffer_blob();
        let off = u32::from_be_bytes(blob[8..12].try_into().unwrap());
        set_be32(&mut blob, 8, off + 2);
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::StructRegionInvalid));
    }

    #[test]
    fn a_struct_region_pointing_into_the_header_is_refused() {
        let mut blob = framebuffer_blob();
        set_be32(&mut blob, 8, 8);
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::StructRegionInvalid));
    }

    #[test]
    fn a_struct_region_running_past_totalsize_is_refused() {
        let mut blob = framebuffer_blob();
        let len = u32::try_from(blob.len()).unwrap();
        set_be32(&mut blob, 36, len);
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::StructRegionInvalid));
    }

    #[test]
    fn a_strings_region_running_past_totalsize_is_refused() {
        let mut blob = framebuffer_blob();
        let len = u32::try_from(blob.len()).unwrap();
        set_be32(&mut blob, 32, len);
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::StringsRegionInvalid));
    }

    #[test]
    fn overlapping_struct_and_strings_regions_are_refused() {
        let mut blob = framebuffer_blob();
        let off_struct = u32::from_be_bytes(blob[8..12].try_into().unwrap());
        // Point the strings region at the structure region's own bytes.
        set_be32(&mut blob, 12, off_struct);
        set_be32(&mut blob, 32, 8);
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::RegionsOverlap));
    }

    // ---- clause 3: the caps are hard, and each one refuses ----------------

    #[test]
    fn the_caps_are_the_reviewed_constants() {
        // A widened cap must be a diff in a reviewed file, not drift. These
        // pins make the diff appear here too, beside the argument for it.
        assert_eq!(MAX_TOTALSIZE, 256 * 1024);
        assert_eq!(MAX_DEPTH, 16);
        assert_eq!(MAX_TOKENS, 32_768);
    }

    #[test]
    fn nesting_deeper_than_the_cap_is_refused() {
        let mut b = Blob::new();
        for i in 0..=MAX_DEPTH {
            b.begin(if i == 0 { "" } else { "n" });
        }
        for _ in 0..=MAX_DEPTH {
            b.end();
        }
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::DepthOverCap));
    }

    #[test]
    fn a_token_flood_is_refused_inside_the_size_cap() {
        let mut b = Blob::new();
        b.begin("");
        for _ in 0..MAX_TOKENS {
            b.token(FDT_NOP);
        }
        b.end();
        b.fdt_end();
        let blob = b.build();
        assert!(
            u32::try_from(blob.len()).unwrap() <= MAX_TOTALSIZE,
            "the flood must fit under MAX_TOTALSIZE or this cap is untestable"
        );
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::TokenOverCap));
    }

    // ---- clause 4: the structure walk is total ----------------------------

    #[test]
    fn an_unknown_token_is_refused_not_skipped() {
        let mut b = Blob::new();
        b.begin("");
        b.token(7);
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::TokenUnknown));
    }

    #[test]
    fn a_region_ending_mid_token_is_refused() {
        let mut b = Blob::new();
        b.begin("");
        b.token(FDT_PROP); // ...and no len/nameoff/value follows
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::StructTruncated));
    }

    #[test]
    fn a_node_name_with_no_terminator_is_refused() {
        let mut b = Blob::new();
        b.token(FDT_BEGIN_NODE);
        b.structure.extend_from_slice(b"noterm__"); // 8 bytes, aligned, no NUL
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::NameUnterminated));
    }

    #[test]
    fn a_property_length_running_past_the_region_is_refused() {
        let mut b = Blob::new();
        b.begin("");
        let nameoff = b.string_off("compatible");
        b.token(FDT_PROP);
        b.structure.extend_from_slice(&0x1000u32.to_be_bytes()); // lying len
        b.structure.extend_from_slice(&nameoff.to_be_bytes());
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::PropOutOfBounds));
    }

    #[test]
    fn a_property_name_offset_outside_the_strings_region_is_refused() {
        let mut b = Blob::new();
        b.begin("");
        b.prop("compatible", b"simple-framebuffer\0");
        b.end();
        b.fdt_end();
        let mut blob = b.build();
        // Shrink the strings region so the (only) name now sits outside it.
        set_be32(&mut blob, 32, 0);
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::PropNameInvalid));
    }

    #[test]
    fn a_property_name_unterminated_within_the_strings_region_is_refused() {
        let mut b = Blob::new();
        b.begin("");
        b.prop("compatible", b"simple-framebuffer\0");
        b.end();
        b.fdt_end();
        let mut blob = b.build();
        // Cut the strings region one byte short of the name's terminator.
        let strings_len = u32::from_be_bytes(blob[32..36].try_into().unwrap());
        set_be32(&mut blob, 32, strings_len - 1);
        // The blob still holds the byte, but the REGION no longer does.
        let shortened = u32::try_from(blob.len()).unwrap() - 1;
        set_be32(&mut blob, 4, shortened);
        blob.truncate(blob.len() - 1);
        assert_eq!(simple_framebuffer_reg(&blob), Err(Refusal::PropNameInvalid));
    }

    #[test]
    fn an_end_node_with_nothing_open_is_refused() {
        let mut b = Blob::new();
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::StructureMalformed));
    }

    #[test]
    fn an_end_with_nodes_still_open_is_refused() {
        let mut b = Blob::new();
        b.begin("");
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::StructureMalformed));
    }

    #[test]
    fn a_property_outside_any_node_is_refused() {
        let mut b = Blob::new();
        b.prop("compatible", b"x\0");
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::StructureMalformed));
    }

    #[test]
    fn a_region_exhausted_without_fdt_end_is_refused() {
        let mut b = Blob::new();
        b.begin("");
        b.end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::StructTruncated));
    }

    // ---- clause 5: the one fact, exactly ----------------------------------

    #[test]
    fn the_fact_is_extracted_under_root_defaults() {
        assert_eq!(
            simple_framebuffer_reg(&framebuffer_blob()),
            Ok(FbReg { base: 0x1eaa_9000, size: 0x007f_8000 })
        );
    }

    #[test]
    fn explicit_two_by_two_cells_are_honoured() {
        let mut b = Blob::new();
        b.begin("");
        b.prop_cells("#address-cells", &[2]);
        b.prop_cells("#size-cells", &[2]);
        b.begin("framebuffer@10_00000000");
        b.prop("compatible", b"simple-framebuffer\0");
        b.prop_cells("reg", &[0x10, 0x0000_0000, 0x0, 0x0040_0000]);
        b.end();
        b.end();
        b.fdt_end();
        assert_eq!(
            simple_framebuffer_reg(&b.build()),
            Ok(FbReg { base: 0x10_0000_0000, size: 0x0040_0000 })
        );
    }

    #[test]
    fn cells_wider_than_two_are_refused_not_truncated() {
        let mut b = Blob::new();
        b.begin("");
        b.prop_cells("#address-cells", &[3]);
        b.begin("fb");
        b.prop("compatible", b"simple-framebuffer\0");
        b.prop_cells("reg", &[0, 0, 0, 1]);
        b.end();
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::CellsUnsupported));
    }

    #[test]
    fn a_cells_property_that_is_not_four_bytes_is_refused() {
        let mut b = Blob::new();
        b.begin("");
        b.prop("#address-cells", &[0, 2]);
        b.begin("fb");
        b.prop("compatible", b"simple-framebuffer\0");
        b.prop_cells("reg", &[0, 0, 1]);
        b.end();
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::CellsUnsupported));
    }

    #[test]
    fn a_matched_node_without_reg_is_a_refusal_not_a_skip() {
        let mut b = Blob::new();
        b.begin("");
        b.begin("fb");
        b.prop("compatible", b"simple-framebuffer\0");
        b.end();
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::RegAbsent));
    }

    #[test]
    fn a_reg_too_short_for_one_pair_is_refused() {
        let mut b = Blob::new();
        b.begin("");
        b.begin("fb");
        b.prop("compatible", b"simple-framebuffer\0");
        b.prop_cells("reg", &[0x1eaa_9000, 0x007f_8000]); // 2 cells; 3 needed
        b.end();
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::RegMalformed));
    }

    #[test]
    fn compatible_matches_whole_entries_only() {
        let mut b = Blob::new();
        b.begin("");
        b.begin("fb");
        b.prop("compatible", b"simple-framebuffer-extended\0");
        b.prop_cells("reg", &[0, 0x1000, 0x1000]);
        b.end();
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::NodeAbsent));
    }

    #[test]
    fn a_multi_entry_compatible_containing_the_exact_entry_matches() {
        let mut b = Blob::new();
        b.begin("");
        b.begin("fb");
        b.prop("compatible", b"vendor,fancy-fb\0simple-framebuffer\0");
        b.prop_cells("reg", &[0, 0x2000, 0x1000]);
        b.end();
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Ok(FbReg { base: 0x2000, size: 0x1000 }));
    }

    #[test]
    fn the_first_completed_match_wins_deterministically() {
        let mut b = Blob::new();
        b.begin("");
        b.begin("fb-first");
        b.prop("compatible", b"simple-framebuffer\0");
        b.prop_cells("reg", &[0, 0x1000, 0x100]);
        b.end();
        b.begin("fb-second");
        b.prop("compatible", b"simple-framebuffer\0");
        b.prop_cells("reg", &[0, 0x2000, 0x200]);
        b.end();
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Ok(FbReg { base: 0x1000, size: 0x100 }));
    }

    #[test]
    fn no_matching_node_is_an_absence_by_name() {
        let mut b = Blob::new();
        b.begin("");
        b.begin("uart@fe201000");
        b.prop("compatible", b"arm,pl011\0");
        b.end();
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::NodeAbsent));
    }

    #[test]
    fn bytes_beyond_a_truthful_totalsize_are_ignored() {
        let mut blob = framebuffer_blob();
        blob.extend_from_slice(&[0xAA; 64]);
        assert_eq!(
            simple_framebuffer_reg(&blob),
            Ok(FbReg { base: 0x1eaa_9000, size: 0x007f_8000 })
        );
    }

    // ---- clause 6: translation refused, never guessed ---------------------

    #[test]
    fn a_nonempty_ranges_ancestor_is_a_named_refusal() {
        let mut b = Blob::new();
        b.begin("");
        b.prop_cells("#address-cells", &[2]);
        b.prop_cells("#size-cells", &[1]);
        b.begin("soc");
        b.prop_cells("#address-cells", &[2]);
        b.prop_cells("#size-cells", &[1]);
        b.prop_cells("ranges", &[0x0, 0x7c00_0000, 0x0, 0xfc00_0000, 0x0400_0000]);
        b.begin("fb");
        b.prop("compatible", b"simple-framebuffer\0");
        b.prop_cells("reg", &[0, 0x1eaa_9000, 0x007f_8000]);
        b.end();
        b.end();
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Err(Refusal::RangesUnsupported));
    }

    #[test]
    fn an_empty_ranges_ancestor_is_identity() {
        let mut b = Blob::new();
        b.begin("");
        b.begin("soc");
        b.prop("ranges", b"");
        b.begin("fb");
        b.prop("compatible", b"simple-framebuffer\0");
        b.prop_cells("reg", &[0, 0x3000, 0x1000]);
        b.end();
        b.end();
        b.end();
        b.fdt_end();
        assert_eq!(simple_framebuffer_reg(&b.build()), Ok(FbReg { base: 0x3000, size: 0x1000 }));
    }

    // ---- clause 7: linked into nothing the board boots --------------------

    /// The decision's central property, tested as absence rather than argued
    /// as discipline: `BND-03`'s "privileged hostile-parser linked bytes equal
    /// zero" holds because no image crate depends on this one.
    #[test]
    fn no_image_crate_links_this_parser() {
        let os_src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("fdt-walk lives at os/src/fdt-walk")
            .to_path_buf();
        const IMAGE_CRATES: [&str; 9] = [
            "kernel",
            "hal-arm64",
            "hal-x86_64",
            "hal",
            "pi5-image",
            "os",
            "exec",
            "shell",
            "motion",
        ];
        for crate_name in IMAGE_CRATES {
            let manifest = os_src.join(crate_name).join("Cargo.toml");
            let contents = std::fs::read_to_string(&manifest)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", manifest.display()));
            assert!(
                !contents.contains("fdt-walk"),
                "{crate_name} depends on fdt-walk — the containment decision in \
                 STORY-P1-13-01 keeps this parser OUT of every image; consuming it \
                 means a real C4 domain or the off-board host, never a link"
            );
        }
    }
}
