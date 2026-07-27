//! Read-only PCI configuration-space enumeration (`STORY-P0-04-03`, Goal
//! `G-HW-4`).
//!
//! Structured like `idt`/`interrupts` split one file instead of two: the
//! pure, host-testable parts — [`BdfAddress`] validation, legacy CAM
//! address-word packing ([`config_address`]), config-header field decoding,
//! and the [`enumerate_bus_zero`] walk itself, generic over the
//! [`ConfigSpace`] read abstraction — carry `#[cfg(test)]` unit tests and
//! compile everywhere, while the one piece needing real CPU semantics
//! ([`PortCam`]'s `in`/`out` port I/O against `0xCF8`/`0xCFC`) is gated to
//! `not(target_os = "windows")` like `interrupts` is and is proved only by
//! the Tier 0 fixture (`TEST-P0-04-03-A`).
//!
//! **Read-only by construction.** [`ConfigSpace`] exposes exactly one
//! operation: a 32-bit configuration read. There is no write method to
//! misuse, so nothing built on this abstraction can mutate device state —
//! Interface Segregation doing containment work, per
//! `agent/CODING_STANDARDS.md`. The only write the [`PortCam`] backend ever
//! issues is to the host bridge's `CONFIG_ADDRESS` register (`0xCF8`),
//! which is the legacy CAM *selection mechanism itself*, not device
//! configuration state.
//!
//! Legacy CAM (`0xCF8`/`0xCFC` port I/O) is deliberately chosen over
//! MMIO ECAM for this Story: it needs no MCFG parsing and no boot-time
//! page-table extension (q35's ECAM window at `0xB0000000` is outside
//! `boot.rs`'s identity maps), keeping a "minimal bus-enumeration pass"
//! actually minimal. ECAM is named, not silently solved, in
//! `STORY-P0-04-03`.

use hal::device::{DeviceDescriptor, DeviceTable, DeviceTableError};

/// Devices per PCI bus: the CAM address word gives the device field 5 bits.
pub const MAX_DEVICES_PER_BUS: u8 = 32;

/// Functions per PCI device: the CAM address word gives the function field
/// 3 bits.
pub const MAX_FUNCTIONS_PER_DEVICE: u8 = 8;

/// The vendor id an absent function reads back on every PCI implementation
/// (the bus floats high on an unclaimed configuration read).
pub const VENDOR_ABSENT: u16 = 0xFFFF;

/// Config-space offset of the vendor-id/device-id dword.
pub const OFFSET_VENDOR_DEVICE: u8 = 0x00;

/// Config-space offset of the dword whose bits 16..=23 are the header-type
/// byte (bit 23 = multifunction flag).
pub const OFFSET_HEADER_TYPE: u8 = 0x0C;

/// A validated bus/device/function topology position.
///
/// Construction is the validation boundary: [`BdfAddress::new`] refuses
/// out-of-range device/function numbers, so every downstream consumer
/// (address packing, the enumeration walk, [`ConfigSpace`] backends) can
/// rely on the fields being in range without re-checking — a newtype doing
/// the range-proof once, per `agent/CODING_STANDARDS.md`'s style notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BdfAddress {
    bus: u8,
    device: u8,
    function: u8,
}

impl BdfAddress {
    /// Creates a validated position, or `None` if `device` or `function`
    /// exceeds its CAM field width ([`MAX_DEVICES_PER_BUS`] /
    /// [`MAX_FUNCTIONS_PER_DEVICE`]). Every `bus` value is valid: the CAM
    /// bus field is a full 8 bits.
    pub const fn new(bus: u8, device: u8, function: u8) -> Option<Self> {
        if device >= MAX_DEVICES_PER_BUS || function >= MAX_FUNCTIONS_PER_DEVICE {
            return None;
        }
        Some(BdfAddress { bus, device, function })
    }

    /// Bus number.
    pub const fn bus(self) -> u8 {
        self.bus
    }

    /// Device number (always `< 32`, guaranteed at construction).
    pub const fn device(self) -> u8 {
        self.device
    }

    /// Function number (always `< 8`, guaranteed at construction).
    pub const fn function(self) -> u8 {
        self.function
    }
}

/// Packs a legacy CAM `CONFIG_ADDRESS` word: enable bit 31, bus 23..=16,
/// device 15..=11, function 10..=8, dword-aligned register offset 7..=2.
///
/// `offset` is aligned down to the containing dword (bits 1..=0 are
/// reserved-zero in the register) — configuration reads through the legacy
/// mechanism are whole-dword reads by design, so an unaligned offset means
/// "the dword containing this byte", not an error.
pub const fn config_address(bdf: BdfAddress, offset: u8) -> u32 {
    0x8000_0000
        | (bdf.bus as u32) << 16
        | (bdf.device as u32) << 11
        | (bdf.function as u32) << 8
        | (offset & 0xFC) as u32
}

/// Vendor id from the [`OFFSET_VENDOR_DEVICE`] dword (low 16 bits).
pub const fn vendor_id(vendor_device_dword: u32) -> u16 {
    vendor_device_dword as u16
}

/// Device id from the [`OFFSET_VENDOR_DEVICE`] dword (high 16 bits).
pub const fn device_id(vendor_device_dword: u32) -> u16 {
    (vendor_device_dword >> 16) as u16
}

/// Multifunction flag from the [`OFFSET_HEADER_TYPE`] dword: bit 7 of the
/// header-type byte (dword bits 16..=23).
pub const fn is_multifunction(header_type_dword: u32) -> bool {
    (header_type_dword >> 16) & 0x80 != 0
}

/// Read-only access to PCI configuration space.
///
/// Exactly one method, and it reads — see the module doc for why the write
/// operation is structurally absent rather than merely unused. Implementors:
/// [`PortCam`] (real legacy CAM port I/O, Tier 0 / target only) and the
/// `#[cfg(test)]` mock configuration spaces in this module's host tests.
pub trait ConfigSpace {
    /// Reads the aligned 32-bit configuration dword containing `offset` for
    /// the function at `bdf`. An absent function must read as all-ones
    /// (`0xFFFF_FFFF`), per PCI's unclaimed-read semantics.
    fn read_dword(&mut self, bdf: BdfAddress, offset: u8) -> u32;
}

/// Walks bus 0's 32 device slots and records every present function into
/// `table`, in (device, function) order.
///
/// For each device slot, function 0 is probed first; functions 1..=7 are
/// probed only when function 0's header-type byte reports multifunction —
/// per the PCI specification, single-function devices are not required to
/// decode nonzero function numbers, so probing them anyway would read
/// aliased garbage on real hardware. The walk is bounded by construction
/// (at most 32 × 8 probe positions, two dword reads each) and performs
/// configuration *reads only* (see [`ConfigSpace`]).
///
/// Fails closed with [`DeviceTableError::Full`] if the bus presents more
/// functions than `table`'s capacity `N`; entries recorded before the
/// overflow remain valid, and the table is never silently truncated into a
/// "complete" claim — the caller sees the error and must treat discovery
/// as incomplete.
///
/// Buses behind PCI-to-PCI bridges are out of scope for this Story (named
/// in `STORY-P0-04-03`, not silently solved): QEMU `q35`'s default machine
/// model exposes every device on bus 0.
pub fn enumerate_bus_zero<C: ConfigSpace, const N: usize>(
    config: &mut C,
    table: &mut DeviceTable<N>,
) -> Result<(), DeviceTableError> {
    for device in 0..MAX_DEVICES_PER_BUS {
        // Range invariant: `device < 32` and function `0 < 8`, so `new`
        // cannot fail; unwrap via match keeps this panic-free regardless.
        let Some(function_zero) = BdfAddress::new(0, device, 0) else {
            continue;
        };
        let vendor_device = config.read_dword(function_zero, OFFSET_VENDOR_DEVICE);
        if vendor_id(vendor_device) == VENDOR_ABSENT {
            continue;
        }
        record(table, function_zero, vendor_device)?;

        if is_multifunction(config.read_dword(function_zero, OFFSET_HEADER_TYPE)) {
            for function in 1..MAX_FUNCTIONS_PER_DEVICE {
                let Some(bdf) = BdfAddress::new(0, device, function) else {
                    continue;
                };
                let vendor_device = config.read_dword(bdf, OFFSET_VENDOR_DEVICE);
                if vendor_id(vendor_device) == VENDOR_ABSENT {
                    continue;
                }
                record(table, bdf, vendor_device)?;
            }
        }
    }
    Ok(())
}

/// Records one present function into `table`, propagating capacity
/// exhaustion unchanged.
fn record<const N: usize>(
    table: &mut DeviceTable<N>,
    bdf: BdfAddress,
    vendor_device: u32,
) -> Result<(), DeviceTableError> {
    table.push(DeviceDescriptor {
        bus: bdf.bus(),
        device: bdf.device(),
        function: bdf.function(),
        vendor_id: vendor_id(vendor_device),
        device_id: device_id(vendor_device),
    })
}

/// The legacy CAM (`0xCF8`/`0xCFC`) [`ConfigSpace`] backend.
///
/// Gated off Windows hosts alongside `interrupts` (its `in`/`out` `asm!`
/// has no ELF-specific content, matching `qemu_exit`'s
/// gated-for-consistency rationale) and proved only under Tier 0.
#[cfg(not(target_os = "windows"))]
pub struct PortCam(());

#[cfg(not(target_os = "windows"))]
impl PortCam {
    /// The legacy CAM `CONFIG_ADDRESS` register.
    const CONFIG_ADDRESS_PORT: u16 = 0xCF8;
    /// The legacy CAM `CONFIG_DATA` window.
    const CONFIG_DATA_PORT: u16 = 0xCFC;

    /// Creates the port-CAM backend.
    ///
    /// # Safety
    ///
    /// The caller must guarantee exclusive use of I/O ports
    /// `0xCF8`/`0xCFC` for as long as the returned value is used: the
    /// address-then-data register pair is a single shared hardware resource,
    /// and an interleaved writer between the address write and the data read
    /// would redirect the read. On this Phase's single-CPU boot path with no
    /// other config-space user, that holds trivially.
    pub const unsafe fn new() -> Self {
        PortCam(())
    }
}

#[cfg(not(target_os = "windows"))]
impl ConfigSpace for PortCam {
    fn read_dword(&mut self, bdf: BdfAddress, offset: u8) -> u32 {
        let address = config_address(bdf, offset);
        let value: u32;
        // SAFETY: `new`'s contract gives this backend exclusive use of the
        // 0xCF8/0xCFC register pair, so the address write below is observed
        // by the data read with no interleaving; both instructions touch
        // only those I/O ports (no memory), and a configuration *read* has
        // no device-state side effect to account for.
        unsafe {
            core::arch::asm!(
                "out dx, eax",
                in("dx") Self::CONFIG_ADDRESS_PORT,
                in("eax") address,
                options(nomem, nostack, preserves_flags)
            );
            core::arch::asm!(
                "in eax, dx",
                in("dx") Self::CONFIG_DATA_PORT,
                out("eax") value,
                options(nomem, nostack, preserves_flags)
            );
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock configuration space: a fixed set of (bdf, dword-offset) → value
    /// entries, all-ones everywhere else, recording every read it serves so
    /// tests can assert exactly what the walk touched.
    struct MockConfigSpace {
        entries: std::vec::Vec<(BdfAddress, u8, u32)>,
        reads: std::vec::Vec<(BdfAddress, u8)>,
    }

    impl MockConfigSpace {
        fn new(entries: std::vec::Vec<(BdfAddress, u8, u32)>) -> Self {
            MockConfigSpace { entries, reads: std::vec::Vec::new() }
        }
    }

    impl ConfigSpace for MockConfigSpace {
        fn read_dword(&mut self, bdf: BdfAddress, offset: u8) -> u32 {
            let offset = offset & 0xFC;
            self.reads.push((bdf, offset));
            self.entries
                .iter()
                .find(|(b, o, _)| *b == bdf && *o == offset)
                .map(|(_, _, v)| *v)
                .unwrap_or(0xFFFF_FFFF)
        }
    }

    fn bdf(device: u8, function: u8) -> BdfAddress {
        BdfAddress::new(0, device, function).unwrap()
    }

    /// vendor/device dword for (vendor, device) ids.
    fn ids(vendor: u16, device: u16) -> u32 {
        (device as u32) << 16 | vendor as u32
    }

    /// Header-type dword with the multifunction bit set/clear.
    fn header(multifunction: bool) -> u32 {
        if multifunction {
            0x80 << 16
        } else {
            0
        }
    }

    #[test]
    fn bdf_construction_rejects_out_of_range_fields() {
        assert!(BdfAddress::new(0, 31, 7).is_some());
        assert!(BdfAddress::new(255, 0, 0).is_some());
        assert!(BdfAddress::new(0, 32, 0).is_none());
        assert!(BdfAddress::new(0, 0, 8).is_none());
    }

    #[test]
    fn config_address_packs_the_documented_cam_layout() {
        // Worked example: bus 0, device 31, function 3, offset 0x00 —
        // q35's SMBus function position.
        assert_eq!(config_address(bdf(31, 3), 0x00), 0x8000_FB00);
        // Enable bit always set; offset aligned down to the dword.
        assert_eq!(config_address(bdf(0, 0), 0x0E), 0x8000_000C);
        assert_eq!(
            config_address(BdfAddress::new(0xAB, 0x1F, 0x07).unwrap(), 0xFF),
            0x8000_0000 | 0xAB << 16 | 0x1F << 11 | 0x07 << 8 | 0xFC
        );
    }

    #[test]
    fn header_field_decoding_extracts_the_documented_bits() {
        assert_eq!(vendor_id(0x29C0_8086), 0x8086);
        assert_eq!(device_id(0x29C0_8086), 0x29C0);
        assert!(is_multifunction(0x00_80_00_00));
        assert!(!is_multifunction(0x00_7F_FF_FF));
    }

    #[test]
    fn empty_bus_yields_empty_table_after_a_full_bounded_walk() {
        let mut mock = MockConfigSpace::new(std::vec::Vec::new());
        let mut table: DeviceTable<8> = DeviceTable::new();
        enumerate_bus_zero(&mut mock, &mut table).unwrap();
        assert!(table.is_empty());
        // Exactly one vendor probe per device slot, nothing else: the walk
        // is bounded and doesn't wander into functions of absent devices.
        assert_eq!(mock.reads.len(), MAX_DEVICES_PER_BUS as usize);
        assert!(mock.reads.iter().all(|(b, o)| b.function() == 0 && *o == OFFSET_VENDOR_DEVICE));
    }

    #[test]
    fn present_functions_are_recorded_with_identity_and_position() {
        let mut mock = MockConfigSpace::new(std::vec![
            (bdf(0, 0), OFFSET_VENDOR_DEVICE, ids(0x8086, 0x29C0)),
            (bdf(0, 0), OFFSET_HEADER_TYPE, header(false)),
            (bdf(2, 0), OFFSET_VENDOR_DEVICE, ids(0x1234, 0x1111)),
            (bdf(2, 0), OFFSET_HEADER_TYPE, header(false)),
        ]);
        let mut table: DeviceTable<8> = DeviceTable::new();
        enumerate_bus_zero(&mut mock, &mut table).unwrap();

        let recorded: std::vec::Vec<_> = table.iter().copied().collect();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0].bus, 0);
        assert_eq!(recorded[0].device, 0);
        assert_eq!(recorded[0].function, 0);
        assert_eq!(recorded[0].vendor_id, 0x8086);
        assert_eq!(recorded[0].device_id, 0x29C0);
        assert_eq!(recorded[1].device, 2);
        assert_eq!(recorded[1].vendor_id, 0x1234);
    }

    #[test]
    fn multifunction_devices_have_all_functions_probed() {
        let mut mock = MockConfigSpace::new(std::vec![
            (bdf(31, 0), OFFSET_VENDOR_DEVICE, ids(0x8086, 0x2918)),
            (bdf(31, 0), OFFSET_HEADER_TYPE, header(true)),
            (bdf(31, 2), OFFSET_VENDOR_DEVICE, ids(0x8086, 0x2922)),
            (bdf(31, 3), OFFSET_VENDOR_DEVICE, ids(0x8086, 0x2930)),
        ]);
        let mut table: DeviceTable<8> = DeviceTable::new();
        enumerate_bus_zero(&mut mock, &mut table).unwrap();

        let functions: std::vec::Vec<u8> =
            table.iter().filter(|d| d.device == 31).map(|d| d.function).collect();
        assert_eq!(functions, [0, 2, 3]);
    }

    #[test]
    fn single_function_devices_are_not_probed_beyond_function_zero() {
        // Per-spec containment: a single-function device need not decode
        // nonzero function numbers, so reading them would trust aliased
        // garbage — the walk must never issue those reads at all.
        let mut mock = MockConfigSpace::new(std::vec![
            (bdf(3, 0), OFFSET_VENDOR_DEVICE, ids(0xABCD, 0x0001)),
            (bdf(3, 0), OFFSET_HEADER_TYPE, header(false)),
            // Aliased garbage that would be (wrongly) recorded if probed.
            (bdf(3, 1), OFFSET_VENDOR_DEVICE, ids(0xABCD, 0x0001)),
        ]);
        let mut table: DeviceTable<8> = DeviceTable::new();
        enumerate_bus_zero(&mut mock, &mut table).unwrap();

        assert_eq!(table.len(), 1);
        assert!(mock.reads.iter().all(|(b, _)| !(b.device() == 3 && b.function() != 0)));
    }

    #[test]
    fn walk_reads_only_identity_and_header_type_dwords() {
        let mut mock = MockConfigSpace::new(std::vec![
            (bdf(0, 0), OFFSET_VENDOR_DEVICE, ids(0x8086, 0x29C0)),
            (bdf(0, 0), OFFSET_HEADER_TYPE, header(true)),
        ]);
        let mut table: DeviceTable<8> = DeviceTable::new();
        enumerate_bus_zero(&mut mock, &mut table).unwrap();
        // Discovery-only scope, observed not asserted: no BAR, command,
        // IRQ, or capability-list offset is ever read.
        assert!(mock
            .reads
            .iter()
            .all(|(_, o)| *o == OFFSET_VENDOR_DEVICE || *o == OFFSET_HEADER_TYPE));
    }

    #[test]
    fn capacity_overflow_fails_closed_with_partial_results_intact() {
        let mut mock = MockConfigSpace::new(std::vec![
            (bdf(0, 0), OFFSET_VENDOR_DEVICE, ids(0x8086, 0x29C0)),
            (bdf(0, 0), OFFSET_HEADER_TYPE, header(false)),
            (bdf(1, 0), OFFSET_VENDOR_DEVICE, ids(0x8086, 0x0001)),
            (bdf(1, 0), OFFSET_HEADER_TYPE, header(false)),
            (bdf(2, 0), OFFSET_VENDOR_DEVICE, ids(0x8086, 0x0002)),
            (bdf(2, 0), OFFSET_HEADER_TYPE, header(false)),
        ]);
        let mut table: DeviceTable<2> = DeviceTable::new();
        assert_eq!(enumerate_bus_zero(&mut mock, &mut table), Err(DeviceTableError::Full));
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn enumeration_is_deterministic_across_repeated_walks() {
        let entries = std::vec![
            (bdf(0, 0), OFFSET_VENDOR_DEVICE, ids(0x8086, 0x29C0)),
            (bdf(0, 0), OFFSET_HEADER_TYPE, header(false)),
            (bdf(31, 0), OFFSET_VENDOR_DEVICE, ids(0x8086, 0x2918)),
            (bdf(31, 0), OFFSET_HEADER_TYPE, header(true)),
            (bdf(31, 2), OFFSET_VENDOR_DEVICE, ids(0x8086, 0x2922)),
        ];
        let mut first: DeviceTable<8> = DeviceTable::new();
        let mut second: DeviceTable<8> = DeviceTable::new();
        enumerate_bus_zero(&mut MockConfigSpace::new(entries.clone()), &mut first).unwrap();
        enumerate_bus_zero(&mut MockConfigSpace::new(entries), &mut second).unwrap();
        assert_eq!(first, second);
    }
}
