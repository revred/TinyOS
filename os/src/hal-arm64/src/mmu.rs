//! The flat identity map (`STORY-P1-07-03`): Normal cacheable RAM, Device
//! MMIO, caches on — the prerequisite of measurement, and explicitly *not*
//! the `FEAT-P1-03` port.
//!
//! With `SCTLR_EL1.M == 0` every data access on AArch64 behaves as
//! Device-nGnRnE regardless of what the memory actually is: uncached,
//! unbuffered, no speculation. A dispatch path measured in that state is
//! dominated by DRAM round trips — not slow-but-proportional, *meaningless*
//! (`TEST-P1-07-03-A`). This module exists so that the numbers
//! `STORY-P1-07-06` produces describe TinyOS rather than the memory bus.
//!
//! Two halves, the house split: **descriptor construction is arithmetic** and
//! lives in pure, host-tested functions (`TEST-P1-07-03-A` clause 1 /
//! `SEC-19`); the only board-side `unsafe` is the system-register writes and
//! the cache-maintenance instructions at the bottom.
//!
//! The walk is one L1 (1 GiB blocks) + one L2 for the first GiB (2 MiB
//! blocks) + one L3 for the first 2 MiB (4 KiB pages). The L3 exists to buy
//! two guard holes the linker script has promised this Story since
//! `STORY-P1-07-01`: the page below the boot stack, and page zero. What the
//! map covers is exactly what the image touches — every entry beyond that
//! list is invalid on purpose, because an over-broad map is what makes a
//! wrong attribute invisible (`TEST-P1-07-03-A` clause 2).
//!
//! **Named debt, restated from the Story:** no per-task address spaces, no
//! W^X (every mapped page of the first 2 MiB stays executable at EL1), no
//! `EL0`, no teardown. Nothing here may be cited as isolation evidence.

use crate::board;
use crate::pl011::hex_u64;

// --- attribute encoding (pure) ----------------------------------------------

/// The three memory attributes this map distinguishes, in `MAIR_EL1` index
/// order. There is no fourth on purpose: a bring-up map with more attributes
/// than call sites is a map nobody can check by reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryAttribute {
    /// Normal Write-Back Write-Allocate cacheable, Inner Shareable — RAM.
    NormalWriteBack,
    /// Device-nGnRnE — every MMIO register window.
    DeviceStrict,
    /// Normal Non-Cacheable — the firmware's scan-out framebuffer, which a
    /// device reads behind the CPU's back and which must therefore never sit
    /// dirty in a cache line.
    NormalNonCacheable,
}

impl MemoryAttribute {
    /// The `MAIR_EL1` attribute index this variant occupies.
    #[must_use]
    pub const fn mair_index(self) -> u64 {
        match self {
            MemoryAttribute::NormalWriteBack => 0,
            MemoryAttribute::DeviceStrict => 1,
            MemoryAttribute::NormalNonCacheable => 2,
        }
    }

    /// The shareability field for a descriptor with this attribute: Inner
    /// Shareable for cacheable RAM, outer/none for the rest (the field is
    /// ignored for Device and effectively Outer for Normal Non-Cacheable).
    #[must_use]
    pub const fn shareability(self) -> u64 {
        match self {
            MemoryAttribute::NormalWriteBack => 0b11,
            MemoryAttribute::DeviceStrict | MemoryAttribute::NormalNonCacheable => 0b00,
        }
    }
}

/// `MAIR_EL1`: index 0 Normal WB/WA (`0xFF`), index 1 Device-nGnRnE
/// (`0x00`), index 2 Normal Non-Cacheable (`0x44`).
pub const MAIR_VALUE: u64 = 0x0000_0000_0044_00FF;

/// `TCR_EL1`: 39-bit VA (`T0SZ=25`, walk starts at L1), 4 KiB granule,
/// Inner-Shareable Write-Back walks, 40-bit PA (`IPS=0b010`), `TTBR1` walks
/// disabled (`EPD1=1`). 39 bits reach `0x1F_0000_0000 + 4 MiB` — the RP1
/// window, the highest CPU address this image touches — with headroom, and
/// keep the walk at three levels.
pub const TCR_VALUE: u64 = 0x0000_0002_0099_3519;

/// The `SCTLR_EL1` bits the switch sets: `M` (translation), `C` (data
/// cache), `I` (instruction cache).
pub const SCTLR_ENABLE_BITS: u64 = (1 << 0) | (1 << 2) | (1 << 12);

// --- descriptor construction (pure) -----------------------------------------

const VALID: u64 = 1 << 0;
const TABLE_OR_PAGE: u64 = 1 << 1;
const ACCESS_FLAG: u64 = 1 << 10;
const UXN: u64 = 1 << 54;
const PXN: u64 = 1 << 53;

/// A next-level table descriptor.
#[must_use]
pub const fn table_descriptor(next_table_pa: u64) -> u64 {
    next_table_pa | VALID | TABLE_OR_PAGE
}

/// A block descriptor (1 GiB at L1, 2 MiB at L2). `EL0` never exists in this
/// slice, so `UXN` is always set; `PXN` is clear only where code lives.
#[must_use]
pub const fn block_descriptor(pa: u64, attribute: MemoryAttribute, executable: bool) -> u64 {
    descriptor_common(pa, attribute, executable)
}

/// A 4 KiB page descriptor (L3) — a block plus the level-3 type bit.
#[must_use]
pub const fn page_descriptor(pa: u64, attribute: MemoryAttribute, executable: bool) -> u64 {
    descriptor_common(pa, attribute, executable) | TABLE_OR_PAGE
}

const fn descriptor_common(pa: u64, attribute: MemoryAttribute, executable: bool) -> u64 {
    let pxn = if executable { 0 } else { PXN };
    pa | VALID
        | (attribute.mair_index() << 2)
        | (attribute.shareability() << 8)
        | ACCESS_FLAG
        | UXN
        | pxn
}

// --- the map itself (pure) ---------------------------------------------------

/// Entries per translation table at every level with a 4 KiB granule.
pub const TABLE_ENTRIES: usize = 512;

/// The page deliberately left unmapped at virtual address zero, so a null
/// dereference reports through `STORY-P1-07-02`'s handler instead of reading
/// firmware memory.
pub const NULL_GUARD_PAGE: usize = 0;

/// Fills the three tables with the identity map. Pure: the tables and the
/// physical addresses they will be installed at are inputs, so the exact
/// descriptor words are host-testable (`TEST-P1-07-03-A` clause 1).
///
/// The map, in full — and nothing else is valid:
///
/// - `L1[0]` → the L2 table (first GiB needs 2 MiB granularity for the
///   framebuffer's attribute island).
/// - `L1[1]` → 1 GiB Normal WB block (the rest of the 2 GiB minimum RAM).
/// - `L1[64]`, `L1[65]` → Device blocks: the BCM2712 SoC peripheral space
///   (PCIe2 controller, debug UART, STAT GPIO, VideoCore mailbox).
/// - `L1[124]` → Device block: the RP1 window at `0x1F_0000_0000`.
/// - `L2[0]` → the L3 table (first 2 MiB needs 4 KiB granularity for the
///   guard holes).
/// - `L2[1..]` → 2 MiB Normal WB blocks, except the blocks the firmware's
///   scan-out framebuffer occupies, which are Normal Non-Cacheable.
/// - `L3[..]` → 4 KiB Normal WB pages, executable (this is where the image
///   lives; W^X is `FEAT-P1-03`'s debt, named), except [`NULL_GUARD_PAGE`]
///   and `stack_guard_page`, which are invalid.
pub fn build_identity_map(
    l1: &mut [u64; TABLE_ENTRIES],
    l2: &mut [u64; TABLE_ENTRIES],
    l3: &mut [u64; TABLE_ENTRIES],
    l2_pa: u64,
    l3_pa: u64,
    stack_guard_page: usize,
) {
    const GIB: u64 = 1 << 30;
    const TWO_MIB: u64 = 1 << 21;
    const PAGE: u64 = 1 << 12;

    l1.fill(0);
    l2.fill(0);
    l3.fill(0);

    // L1: the first GiB walks down to the L2; the second is one WB block.
    l1[0] = table_descriptor(l2_pa);
    l1[1] = block_descriptor(GIB, MemoryAttribute::NormalWriteBack, false);
    // The SoC peripheral space (UART, PCIe2 controller, STAT GPIO, mailbox)
    // spans two 1 GiB entries; the RP1 window sits alone in a third. Derived
    // from the board constants rather than restated, so a moved window moves
    // the map.
    let soc_first = (board::PCIE2_BASE >> 30) as usize;
    let soc_last = (board::STAT_GPIO_BASE >> 30) as usize;
    let mut index = soc_first;
    while index <= soc_last {
        l1[index] = block_descriptor((index as u64) * GIB, MemoryAttribute::DeviceStrict, false);
        index += 1;
    }
    let rp1 = (board::RP1_WINDOW_BASE >> 30) as usize;
    l1[rp1] = block_descriptor((rp1 as u64) * GIB, MemoryAttribute::DeviceStrict, false);

    // L2: the first 2 MiB walks down to the L3; the framebuffer's blocks are
    // the one attribute island in an otherwise uniform WB gigabyte.
    l2[0] = table_descriptor(l3_pa);
    let fb_first = (board::SIMPLEFB_BASE >> 21) as usize;
    let fb_last = ((board::SIMPLEFB_BASE + board::SIMPLEFB_SIZE as u64 - 1) >> 21) as usize;
    for (entry, slot) in l2.iter_mut().enumerate().skip(1) {
        let attribute = if entry >= fb_first && entry <= fb_last {
            MemoryAttribute::NormalNonCacheable
        } else {
            MemoryAttribute::NormalWriteBack
        };
        *slot = block_descriptor((entry as u64) * TWO_MIB, attribute, false);
    }

    // L3: the image's own 2 MiB, 4 KiB pages, executable — minus the two
    // guard holes.
    for (entry, slot) in l3.iter_mut().enumerate() {
        if entry == NULL_GUARD_PAGE || entry == stack_guard_page {
            continue;
        }
        *slot = page_descriptor((entry as u64) * PAGE, MemoryAttribute::NormalWriteBack, true);
    }
}

/// Walks the (host-side) tables exactly as the MMU would and returns the
/// attribute a load at `va` would see, or `None` for a translation fault.
/// Exists so the tests interrogate the *tables*, not the builder's intent.
#[must_use]
pub fn lookup(
    l1: &[u64; TABLE_ENTRIES],
    l2: &[u64; TABLE_ENTRIES],
    l3: &[u64; TABLE_ENTRIES],
    va: u64,
) -> Option<MemoryAttribute> {
    let l1_entry = l1[((va >> 30) & 0x1FF) as usize];
    if l1_entry & VALID == 0 {
        return None;
    }
    if l1_entry & TABLE_OR_PAGE == 0 {
        return Some(attribute_of(l1_entry));
    }
    let l2_entry = l2[((va >> 21) & 0x1FF) as usize];
    if l2_entry & VALID == 0 {
        return None;
    }
    if l2_entry & TABLE_OR_PAGE == 0 {
        return Some(attribute_of(l2_entry));
    }
    let l3_entry = l3[((va >> 12) & 0x1FF) as usize];
    if l3_entry & VALID == 0 {
        return None;
    }
    Some(attribute_of(l3_entry))
}

/// Decodes the attribute index of a leaf descriptor back to the variant.
/// An index outside the three this map issues is a construction bug, and the
/// walker treats it as such rather than guessing.
const fn attribute_of(descriptor: u64) -> MemoryAttribute {
    match (descriptor >> 2) & 0b111 {
        0 => MemoryAttribute::NormalWriteBack,
        1 => MemoryAttribute::DeviceStrict,
        2 => MemoryAttribute::NormalNonCacheable,
        _ => panic!("a descriptor this map never issues"),
    }
}

// --- the cache-evidence probe (pure over the counter seam) -------------------

/// Words in the probe buffer: 256 KiB — four times a Cortex-A76 L1D, so the
/// loop is memory-bound and the difference it reports is about the cache,
/// not the pipeline (`TEST-P1-07-03-A` clause 4).
pub const PROBE_WORDS: usize = 32_768;

/// Passes over the buffer per probe. More than one, so the cached case is
/// measured warm.
pub const PROBE_PASSES: usize = 4;

/// Reads every word of `buffer` [`PROBE_PASSES`] times under the counter and
/// returns the elapsed ticks. Volatile reads, so the loop survives the
/// optimiser; wrapping arithmetic, so a counter rollover mid-probe returns a
/// small honest number rather than a panic.
pub fn probe_ticks<C: crate::timer::VirtualCounter>(counter: &C, buffer: &[u64]) -> u64 {
    let start = counter.count();
    let mut sum = 0u64;
    for _ in 0..PROBE_PASSES {
        for slot in buffer {
            // SAFETY: `slot` is a live shared reference; a volatile read of
            // it is the same load the optimiser would otherwise be free to
            // hoist out of the pass loop — which is the whole measurement.
            sum = sum.wrapping_add(unsafe { core::ptr::read_volatile(slot) });
        }
    }
    core::hint::black_box(sum);
    counter.count().wrapping_sub(start)
}

// --- the report line (pure) --------------------------------------------------

/// Capacity of the rendered `TOS64-MMU/1` line.
pub const LINE_CAPACITY: usize = 96;

/// Renders the one-line MMU verdict: the `SCTLR_EL1` readback and the
/// before/after probe, `TOS64-MMU/1 sctlr=<hex16> off=<ticks> on=<ticks>\n`.
/// The framer owns the CR, like every report line in this crate.
#[must_use]
pub fn report_line(
    sctlr_readback: u64,
    off_ticks: u64,
    on_ticks: u64,
) -> ([u8; LINE_CAPACITY], usize) {
    let mut line = [0u8; LINE_CAPACITY];
    let mut len = 0;
    push(&mut line, &mut len, b"TOS64-MMU/1 sctlr=");
    push(&mut line, &mut len, &hex_u64(sctlr_readback));
    push(&mut line, &mut len, b" off=");
    push_decimal(&mut line, &mut len, off_ticks);
    push(&mut line, &mut len, b" on=");
    push_decimal(&mut line, &mut len, on_ticks);
    push(&mut line, &mut len, b"\n");
    (line, len)
}

fn push(line: &mut [u8; LINE_CAPACITY], len: &mut usize, bytes: &[u8]) {
    for &byte in bytes {
        if *len < LINE_CAPACITY {
            line[*len] = byte;
            *len += 1;
        }
    }
}

fn push_decimal(line: &mut [u8; LINE_CAPACITY], len: &mut usize, value: u64) {
    let mut digits = [0u8; 20];
    let mut count = 0;
    let mut rest = value;
    loop {
        digits[count] = b'0' + (rest % 10) as u8;
        rest /= 10;
        count += 1;
        if rest == 0 {
            break;
        }
    }
    while count > 0 {
        count -= 1;
        push(line, len, &[digits[count]]);
    }
}

// --- aarch64 glue: the tables, the switch, the maintenance -------------------
//
// Everything below is the thin register half `TEST-P1-07-03-A` clause 1
// scopes the board-side `unsafe` to. It is compiled only for AArch64 and is
// exercised by the Tier 1 run, never by host tests.

/// A 4 KiB-aligned translation table, the shape the walk hardware demands.
#[cfg(target_arch = "aarch64")]
#[repr(C, align(4096))]
struct TranslationTable([u64; TABLE_ENTRIES]);

#[cfg(target_arch = "aarch64")]
static mut L1_TABLE: TranslationTable = TranslationTable([0; TABLE_ENTRIES]);
#[cfg(target_arch = "aarch64")]
static mut L2_TABLE: TranslationTable = TranslationTable([0; TABLE_ENTRIES]);
#[cfg(target_arch = "aarch64")]
static mut L3_TABLE: TranslationTable = TranslationTable([0; TABLE_ENTRIES]);

/// The probe buffer, in `.bss` like everything else static here — 256 KiB
/// the boot stub zeroes before Rust runs.
#[cfg(target_arch = "aarch64")]
static mut PROBE_BUFFER: [u64; PROBE_WORDS] = [0; PROBE_WORDS];

#[cfg(target_arch = "aarch64")]
extern "C" {
    /// The 4 KiB the linker reserved below the boot stack
    /// (`targets/aarch64-tinyos.ld`); left unmapped so a stack overflow
    /// reports through `STORY-P1-07-02`'s handler instead of eating `.bss`.
    static __stack_guard: u8;
}

/// Runs the cache-evidence probe against the real counter
/// (`TEST-P1-07-03-A` clause 4). Called once before the switch and once
/// after; the pair of numbers is the Story's acceptance evidence.
#[cfg(target_arch = "aarch64")]
pub fn measure_cache_probe() -> u64 {
    // SAFETY: single core, and the probe only reads; the raw pointer round
    // trip avoids holding a reference to a mutable static.
    let buffer: &[u64] = unsafe {
        core::slice::from_raw_parts(core::ptr::addr_of!(PROBE_BUFFER).cast(), PROBE_WORDS)
    };
    probe_ticks(&crate::timer::SystemRegisters, buffer)
}

/// Builds the identity map into the static tables and turns translation and
/// both caches on. Returns the `SCTLR_EL1` readback — belief comes from the
/// register, not from the write (the house rule since `STORY-P1-09-09`).
///
/// # Safety
///
/// Must run at `EL1`, on the boot core, exactly once, with the vector table
/// already installed (`STORY-P1-07-03` depends on `-02` precisely so a wrong
/// table faults loudly). The map covers every address the image touches; the
/// caller must not have live pointers into anything the map excludes.
#[cfg(target_arch = "aarch64")]
pub unsafe fn enable_identity_map() -> u64 {
    // SAFETY: single core, called once, and the tables are not yet live —
    // the walk starts only when TTBR0/SCTLR are written below.
    let (l1_pa, l2_pa, l3_pa) = unsafe {
        let l1 = core::ptr::addr_of_mut!(L1_TABLE);
        let l2 = core::ptr::addr_of_mut!(L2_TABLE);
        let l3 = core::ptr::addr_of_mut!(L3_TABLE);
        let guard_page = (core::ptr::addr_of!(__stack_guard) as u64 >> 12) as usize;
        build_identity_map(
            &mut (*l1).0,
            &mut (*l2).0,
            &mut (*l3).0,
            l2 as u64,
            l3 as u64,
            guard_page,
        );
        (l1 as u64, l2 as u64, l3 as u64)
    };
    let _ = (l2_pa, l3_pa);

    // SAFETY: the register sequence `TEST-P1-07-03-A` clause 3 specifies —
    // attributes and walk configuration first, TLB invalidated, barriers
    // between every dependent step, and only then the enable. The tables the
    // walk will read were fully written above and are made visible by the
    // `dsb ish` before the enable.
    unsafe {
        core::arch::asm!(
            "msr mair_el1, {mair}",
            "msr tcr_el1, {tcr}",
            "msr ttbr0_el1, {ttbr}",
            "dsb ish",
            "tlbi vmalle1",
            "dsb ish",
            "isb",
            mair = in(reg) MAIR_VALUE,
            tcr = in(reg) TCR_VALUE,
            ttbr = in(reg) l1_pa,
            options(nostack, preserves_flags),
        );
    }

    let mut sctlr: u64;
    // SAFETY: a side-effect-free read.
    unsafe {
        core::arch::asm!("mrs {v}, sctlr_el1", v = out(reg) sctlr, options(nomem, nostack));
    }
    sctlr |= SCTLR_ENABLE_BITS;
    // SAFETY: the switch itself. The code executing this sequence is
    // identity-mapped executable (the L3 covers the image), the stack is
    // identity-mapped RAM, and the `isb` makes the new context effective
    // before the next instruction fetch completes.
    unsafe {
        core::arch::asm!(
            "msr sctlr_el1, {v}",
            "isb",
            v = in(reg) sctlr,
            options(nostack, preserves_flags),
        );
    }
    let readback: u64;
    // SAFETY: a side-effect-free read; this is the value the report line
    // carries and the self-check believes.
    unsafe {
        core::arch::asm!("mrs {v}, sctlr_el1", v = out(reg) readback, options(nomem, nostack));
    }
    readback
}

/// A Cortex-A76 data-cache line. Read from the architecture's own
/// `CTR_EL0.DminLine` this would be discovered; pinned instead, because a
/// wrong constant here degrades to extra maintenance, never to corruption
/// (the loop rounds outward).
#[cfg(target_arch = "aarch64")]
pub const DCACHE_LINE_BYTES: usize = 64;

/// Cleans (writes back) every data-cache line overlapping `[start, start+len)`
/// to the point of coherency, then a full barrier. The transmit half of DMA
/// coherency: what the CPU wrote, a bus master must read.
#[cfg(target_arch = "aarch64")]
pub fn clean_dcache_range(start: usize, len: usize) {
    maintain_range(start, len, false);
}

/// Cleans **and invalidates** the lines — the exchange half: what the CPU
/// wrote goes out, and the CPU's next read comes from memory a device may
/// have written meanwhile.
#[cfg(target_arch = "aarch64")]
pub fn clean_invalidate_dcache_range(start: usize, len: usize) {
    maintain_range(start, len, true);
}

#[cfg(target_arch = "aarch64")]
fn maintain_range(start: usize, len: usize, invalidate: bool) {
    if len == 0 {
        return;
    }
    let first = start & !(DCACHE_LINE_BYTES - 1);
    let last = (start + len - 1) & !(DCACHE_LINE_BYTES - 1);
    let mut line = first;
    loop {
        // SAFETY: `dc cvac`/`dc civac` by VA on addresses inside a buffer
        // the caller owns; both are permitted at EL1 and, with the MMU off,
        // are no-ops that fault nothing.
        unsafe {
            if invalidate {
                core::arch::asm!("dc civac, {a}", a = in(reg) line, options(nostack, preserves_flags));
            } else {
                core::arch::asm!("dc cvac, {a}", a = in(reg) line, options(nostack, preserves_flags));
            }
        }
        if line == last {
            break;
        }
        line += DCACHE_LINE_BYTES;
    }
    // SAFETY: the barrier that orders the maintenance above before whatever
    // MMIO write hands the buffer to the device.
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;

    // ---- TEST-P1-07-03-A clause 1: encoding is arithmetic, pinned ---------

    #[test]
    fn mair_pins_the_three_attributes_in_index_order() {
        assert_eq!(MAIR_VALUE, 0x44_00FF);
        assert_eq!(MemoryAttribute::NormalWriteBack.mair_index(), 0);
        assert_eq!(MemoryAttribute::DeviceStrict.mair_index(), 1);
        assert_eq!(MemoryAttribute::NormalNonCacheable.mair_index(), 2);
        // Attribute bytes, read back out of the value: 0xFF WB/WA, 0x00
        // Device-nGnRnE, 0x44 Normal Non-Cacheable.
        assert_eq!(MAIR_VALUE & 0xFF, 0xFF);
        assert_eq!((MAIR_VALUE >> 8) & 0xFF, 0x00);
        assert_eq!((MAIR_VALUE >> 16) & 0xFF, 0x44);
    }

    #[test]
    fn tcr_pins_the_39_bit_three_level_walk() {
        assert_eq!(TCR_VALUE, 0x0000_0002_0099_3519);
        // Decoded, so a wrong constant fails with a field name.
        assert_eq!(TCR_VALUE & 0x3F, 25, "T0SZ: 39-bit VA");
        assert_eq!((TCR_VALUE >> 8) & 0b11, 0b01, "IRGN0: WB/WA walks");
        assert_eq!((TCR_VALUE >> 10) & 0b11, 0b01, "ORGN0: WB/WA walks");
        assert_eq!((TCR_VALUE >> 12) & 0b11, 0b11, "SH0: inner shareable");
        assert_eq!((TCR_VALUE >> 14) & 0b11, 0b00, "TG0: 4 KiB granule");
        assert_eq!((TCR_VALUE >> 23) & 1, 1, "EPD1: no TTBR1 walks");
        assert_eq!((TCR_VALUE >> 32) & 0b111, 0b010, "IPS: 40-bit PA");
        // 39 bits reach the highest address the image touches — a
        // compile-time claim, stated as such (the board.rs convention).
        const { assert!(board::RP1_WINDOW_BASE + board::RP1_WINDOW_MIN_SPAN < (1 << 39)) }
    }

    #[test]
    fn sctlr_enable_bits_are_m_c_and_i_and_nothing_else() {
        assert_eq!(SCTLR_ENABLE_BITS, 0x1005);
    }

    #[test]
    fn descriptors_are_pinned_words() {
        // A next-level table.
        assert_eq!(table_descriptor(0x8_0000), 0x8_0003);
        // RAM: Normal WB, Inner Shareable, AF, UXN, PXN (no code there).
        assert_eq!(
            block_descriptor(0x4000_0000, MemoryAttribute::NormalWriteBack, false),
            0x0060_0000_4000_0701
        );
        // Device: attribute index 1, no shareability bits, never executable.
        assert_eq!(
            block_descriptor(0x10_0000_0000, MemoryAttribute::DeviceStrict, false),
            0x0060_0010_0000_0405
        );
        // The framebuffer island: Normal Non-Cacheable, attribute index 2.
        assert_eq!(
            block_descriptor(0x3F80_0000, MemoryAttribute::NormalNonCacheable, false),
            0x0060_0000_3F80_0409
        );
        // A page where code lives: PXN clear, UXN still set (no EL0 exists).
        assert_eq!(
            page_descriptor(0x8_0000, MemoryAttribute::NormalWriteBack, true),
            0x0040_0000_0008_0703
        );
    }

    // ---- TEST-P1-07-03-A clause 2: per-region, explicit, and nothing else --

    type BuiltTables =
        (Box<[u64; TABLE_ENTRIES]>, Box<[u64; TABLE_ENTRIES]>, Box<[u64; TABLE_ENTRIES]>);

    fn built() -> BuiltTables {
        let mut l1 = Box::new([0u64; TABLE_ENTRIES]);
        let mut l2 = Box::new([0u64; TABLE_ENTRIES]);
        let mut l3 = Box::new([0u64; TABLE_ENTRIES]);
        // Table addresses and the guard page a linker could plausibly emit;
        // the builder must place them in the descriptors verbatim.
        build_identity_map(&mut l1, &mut l2, &mut l3, 0x9_0000, 0xA_0000, 0xB0);
        (l1, l2, l3)
    }

    #[test]
    fn every_region_the_image_touches_translates_with_its_declared_attribute() {
        let (l1, l2, l3) = built();
        let device = [
            board::DEBUG_UART_BASE,
            board::PCIE2_BASE,
            board::PCIE2_BASE + board::PCIE2_SIZE as u64 - 4,
            board::STAT_GPIO_BASE,
            board::RP1_WINDOW_BASE,
            board::RP1_WINDOW_BASE + board::RP1_WINDOW_MIN_SPAN - 4,
            // The VideoCore mailbox (`hdmi.rs`), inside the SoC block.
            0x10_7C01_3880,
        ];
        for address in device {
            assert_eq!(
                lookup(&l1, &l2, &l3, address),
                Some(MemoryAttribute::DeviceStrict),
                "{address:#x} must be Device-nGnRnE"
            );
        }
        let ram = [board::KERNEL_LOAD_ADDRESS, 0x10_0000, 0x4000_0000, board::MIN_RAM_SIZE - 8];
        for address in ram {
            assert_eq!(
                lookup(&l1, &l2, &l3, address),
                Some(MemoryAttribute::NormalWriteBack),
                "{address:#x} must be Normal Write-Back"
            );
        }
        // The scan-out framebuffer: RAM a device reads behind the CPU's back.
        for address in
            [board::SIMPLEFB_BASE, board::SIMPLEFB_BASE + board::SIMPLEFB_SIZE as u64 - 2]
        {
            assert_eq!(
                lookup(&l1, &l2, &l3, address),
                Some(MemoryAttribute::NormalNonCacheable),
                "{address:#x} must be Normal Non-Cacheable"
            );
        }
    }

    #[test]
    fn what_the_image_does_not_touch_faults_by_construction() {
        let (l1, l2, l3) = built();
        // Page zero: a null dereference reports, never reads firmware RAM.
        assert_eq!(lookup(&l1, &l2, &l3, 0), None);
        assert_eq!(lookup(&l1, &l2, &l3, 0xFF8), None);
        // The stack guard page the builder was told about.
        assert_eq!(lookup(&l1, &l2, &l3, 0xB0 << 12), None);
        assert_eq!(lookup(&l1, &l2, &l3, (0xB0 << 12) + 0xFFF), None);
        // Above the minimum RAM this map vouches for.
        assert_eq!(lookup(&l1, &l2, &l3, board::MIN_RAM_SIZE), None);
        // The deliberate-fault fixture's address (`fixture-mmu-fault`).
        assert_eq!(lookup(&l1, &l2, &l3, 0x20_0000_0000), None);
        // The RP1's *bus-side* alias of RAM shares its gigabyte with the
        // PCIe controller's own registers, so it cannot be left unmapped —
        // but a CPU access to it (a bug in the DMA translation arithmetic)
        // must hit Device space, never silently read cacheable RAM.
        assert_eq!(
            lookup(&l1, &l2, &l3, board::RP1_DMA_RAM_BASE),
            Some(MemoryAttribute::DeviceStrict)
        );
    }

    #[test]
    fn the_l1_maps_exactly_five_entries_and_the_declared_tables() {
        let (l1, l2, l3) = built();
        let valid: Vec<usize> = (0..TABLE_ENTRIES).filter(|&index| l1[index] & 1 != 0).collect();
        assert_eq!(valid, vec![0, 1, 64, 65, 124]);
        assert_eq!(l1[0], table_descriptor(0x9_0000));
        assert_eq!(l2[0], table_descriptor(0xA_0000));
        // The L3 has exactly two holes: page zero and the stack guard.
        let l3_invalid: Vec<usize> =
            (0..TABLE_ENTRIES).filter(|&index| l3[index] & 1 == 0).collect();
        assert_eq!(l3_invalid, vec![NULL_GUARD_PAGE, 0xB0]);
        // Code lives in the first 2 MiB and nowhere else: every mapped L3
        // page is executable, every block beyond it is PXN.
        assert_eq!(l3[0x80] & (1 << 53), 0, "the load address is executable");
        assert_ne!(l1[1] & (1 << 53), 0, "RAM above the image is not");
    }

    // ---- TEST-P1-07-03-A clause 4: the probe measures, honestly -----------

    struct ScriptedCounter {
        values: std::cell::RefCell<Vec<u64>>,
    }

    impl crate::timer::VirtualCounter for ScriptedCounter {
        fn count(&self) -> u64 {
            self.values.borrow_mut().remove(0)
        }
    }

    #[test]
    fn the_probe_reads_every_word_and_returns_the_tick_delta() {
        let counter = ScriptedCounter { values: std::cell::RefCell::new(vec![1_000, 43_500]) };
        let buffer = vec![7u64; 64];
        assert_eq!(probe_ticks(&counter, &buffer), 42_500);
    }

    #[test]
    fn a_counter_rollover_mid_probe_reports_small_not_panicking() {
        let counter = ScriptedCounter { values: std::cell::RefCell::new(vec![u64::MAX - 5, 10]) };
        let buffer = vec![0u64; 8];
        assert_eq!(probe_ticks(&counter, &buffer), 16);
    }

    // ---- the report line: exact bytes, framer owns the CR ------------------

    #[test]
    fn the_mmu_line_is_exact_bytes() {
        let (line, len) = report_line(0x30D0_1805, 920_000, 1_800);
        assert_eq!(
            &line[..len],
            b"TOS64-MMU/1 sctlr=0000000030D01805 off=920000 on=1800\n" as &[u8]
        );
    }

    #[test]
    fn no_mmu_line_byte_is_a_carriage_return() {
        let (line, len) = report_line(u64::MAX, u64::MAX, u64::MAX);
        assert!(len <= LINE_CAPACITY);
        assert!(!line[..len].contains(&b'\r'), "the framer owns the CR");
        assert_eq!(line[len - 1], b'\n');
    }
}
