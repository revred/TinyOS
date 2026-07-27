//! `TEST-P0-04-03-A`'s bus-enumeration QEMU fixture: walks PCI bus 0's
//! configuration space through [`hal_x86_64::pci`]'s legacy CAM backend and
//! judges the result against what QEMU's `q35` machine model is known to
//! expose — proving `STORY-P0-04-03` acceptance criterion 1 ("enumeration
//! under QEMU's `q35` model discovers at minimum the host bridge and any
//! devices QEMU's default machine exposes, recorded with vendor/device ID
//! and topology position") against real emulated configuration space, not
//! the host-side mocks `pci`'s own unit tests use.
//!
//! Only reachable when the `fixture-pci-enumeration` feature is enabled —
//! never part of a real boot image.

use hal::device::DeviceTable;
use hal_x86_64::pci::{enumerate_bus_zero, PortCam};
use kernel::capacities::MAX_PCI_DEVICES;

/// Intel's PCI vendor id — what q35's emulated host bridge (82G33/Q35 MCH)
/// reports at position 0:0:0. The *device* id is deliberately not asserted:
/// vendor identity and position prove the walk read real configuration
/// space, without coupling this fixture to one QEMU version's exact chipset
/// model choice.
const HOST_BRIDGE_VENDOR: u16 = 0x8086;

/// Runs the fixture: enumerates bus 0 twice and reports whether (a) the
/// host bridge is present at 0:0:0 with Intel's vendor id, (b) at least one
/// device was discovered at all, and (c) both walks produced identical
/// tables — repeated *reads* of configuration space must be stable, which
/// is also the closest behavioral check this Story has that discovery
/// mutated nothing it later depends on (the structural guarantee is
/// [`hal_x86_64::pci::ConfigSpace`]'s read-only shape itself).
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running (single-CPU boot path,
    // no interrupt handlers touch config space), so it has exclusive use of
    // the 0xCF8/0xCFC register pair — `PortCam::new`'s documented contract.
    let mut cam = unsafe { PortCam::new() };

    let mut first: DeviceTable<MAX_PCI_DEVICES> = DeviceTable::new();
    if enumerate_bus_zero(&mut cam, &mut first).is_err() {
        return false;
    }

    let host_bridge_found = first.iter().any(|d| {
        d.bus == 0 && d.device == 0 && d.function == 0 && d.vendor_id == HOST_BRIDGE_VENDOR
    });
    if !host_bridge_found || first.is_empty() {
        return false;
    }

    let mut second: DeviceTable<MAX_PCI_DEVICES> = DeviceTable::new();
    if enumerate_bus_zero(&mut cam, &mut second).is_err() {
        return false;
    }
    first == second
}
