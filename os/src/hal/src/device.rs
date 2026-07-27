//! Arch-neutral discovered-device model (`STORY-P0-04-03`, Goal `G-HW-4`).
//!
//! [`DeviceTable`] is the bus-enumeration companion to
//! [`crate::topology::Topology`]: the shared output type an x86_64 PCI
//! configuration-space walk (`hal-x86_64::pci`) and a future ARM64 PCIe/ECAM
//! backend (`EPIC-P7`) both produce, so the rest of the kernel consumes one
//! device model regardless of which bus-access mechanism it came from — the
//! same Dependency Inversion translation `topology` established. Fixed
//! capacity, no heap allocation, per the RT discipline in
//! `agent/CODING_STANDARDS.md`.
//!
//! A [`DeviceDescriptor`] records *identity and topology position only* —
//! deliberately no BAR contents, IRQ line, command register, or any other
//! field a driver would act on. Per `FEAT-P0-04`'s containment contract,
//! discovery describes hardware but cannot select a driver or grant DMA,
//! IRQ, or MMIO authority; keeping those fields structurally absent (not
//! merely unused) is how this type enforces that at the type level.

/// One discovered device function as reported by bus enumeration — a PCI
/// configuration-space function on x86_64, or a PCIe/ECAM function on a
/// future ARM64 backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceDescriptor {
    /// Bus number of this function's topology position.
    pub bus: u8,
    /// Device number on `bus` (PCI: 0..=31).
    pub device: u8,
    /// Function number on `device` (PCI: 0..=7).
    pub function: u8,
    /// The vendor identifier the device reports (PCI: config offset 0x00).
    pub vendor_id: u16,
    /// The vendor-assigned device identifier (PCI: config offset 0x02).
    pub device_id: u16,
}

/// Errors mutating a [`DeviceTable`] fails closed with, per
/// `agent/CODING_STANDARDS.md`'s "no stringly-typed errors" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceTableError {
    /// The table already holds `N` entries; no side effects occurred.
    Full,
}

/// Fixed-capacity, arch-neutral table of discovered devices: up to `N`
/// device functions.
///
/// `N` is a caller-chosen capacity bound (see `kernel::capacities`), not a
/// discovered value — a bus presenting more functions than `N` fails closed
/// via [`DeviceTable::push`] returning [`DeviceTableError::Full`] rather
/// than growing unbounded storage, mirroring
/// [`crate::topology::Topology::push`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceTable<const N: usize> {
    devices: [Option<DeviceDescriptor>; N],
    count: usize,
}

impl<const N: usize> DeviceTable<N> {
    /// Creates an empty table. `const fn`: no heap allocation, usable in a
    /// `static` initializer.
    pub const fn new() -> Self {
        DeviceTable { devices: [None; N], count: 0 }
    }

    /// Appends a discovered device function.
    ///
    /// Fails closed with [`DeviceTableError::Full`] and no side effects once
    /// `N` entries are already stored — never panics.
    pub fn push(&mut self, device: DeviceDescriptor) -> Result<(), DeviceTableError> {
        if self.count >= N {
            return Err(DeviceTableError::Full);
        }
        self.devices[self.count] = Some(device);
        self.count += 1;
        Ok(())
    }

    /// The number of device functions currently stored.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Whether no device functions have been stored yet.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Iterates over the stored device functions in discovery order.
    pub fn iter(&self) -> impl Iterator<Item = &DeviceDescriptor> {
        self.devices[..self.count].iter().filter_map(Option::as_ref)
    }
}

impl<const N: usize> Default for DeviceTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(device: u8) -> DeviceDescriptor {
        DeviceDescriptor { bus: 0, device, function: 0, vendor_id: 0x8086, device_id: 0x29C0 }
    }

    #[test]
    fn new_table_is_empty() {
        let table: DeviceTable<4> = DeviceTable::new();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
        assert_eq!(table.iter().count(), 0);
    }

    #[test]
    fn pushed_devices_are_iterated_in_discovery_order() {
        let mut table: DeviceTable<4> = DeviceTable::new();
        table.push(dev(0)).unwrap();
        table.push(dev(2)).unwrap();
        let devices: std::vec::Vec<u8> = table.iter().map(|d| d.device).collect();
        assert_eq!(devices, [0, 2]);
    }

    #[test]
    fn pushing_past_capacity_fails_closed_without_side_effects() {
        let mut table: DeviceTable<2> = DeviceTable::new();
        table.push(dev(0)).unwrap();
        table.push(dev(1)).unwrap();

        assert_eq!(table.push(dev(2)), Err(DeviceTableError::Full));
        // Repeated overflow fails the same way every time, not just once.
        assert_eq!(table.push(dev(3)), Err(DeviceTableError::Full));
        assert_eq!(table.len(), 2);
        let devices: std::vec::Vec<u8> = table.iter().map(|d| d.device).collect();
        assert_eq!(devices, [0, 1]);
    }

    #[test]
    fn descriptor_records_identity_and_position_only() {
        // Structural guard for FEAT-P0-04's containment contract: identity
        // (vendor/device id) plus topology position (bus/device/function) is
        // the whole descriptor — 5 fields totalling 7 meaningful bytes. A
        // field addition that lets discovery carry authority-shaped state
        // (BARs, IRQ lines, command-register contents) grows this size and
        // must consciously revisit this test and the contract it encodes.
        assert_eq!(core::mem::size_of::<DeviceDescriptor>(), 8);
    }
}
