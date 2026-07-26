//! Arch-neutral hardware topology model (`STORY-P0-04-01`, Goal `G-HW-4`).
//!
//! [`Topology`] is the shared output type both an ACPI backend (x86_64,
//! `hal-x86_64`) and a future device-tree backend (ARM64, `EPIC-P7`) produce
//! — the rest of the kernel consumes one topology model regardless of which
//! firmware format it came from, per the Dependency Inversion translation in
//! `agent/CODING_STANDARDS.md`. Fixed-capacity, no heap allocation, per the
//! RT discipline in the same document.

/// One CPU core as reported by firmware — an ACPI MADT Processor Local APIC
/// entry on x86_64, or a `cpu` device-tree node on a future ARM64 backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuDescriptor {
    /// The firmware-assigned processor id (ACPI: MADT's `acpi_processor_id`).
    pub processor_id: u8,
    /// The interrupt-controller id used to target this core (ACPI: the
    /// local APIC id).
    pub interrupt_controller_id: u8,
    /// Whether firmware reports this core as usable. A present-but-disabled
    /// entry (e.g. a core firmware has fused off) is kept in the topology
    /// rather than silently dropped, so a caller can distinguish "this core
    /// doesn't exist" from "this core exists but isn't usable".
    pub enabled: bool,
}

/// Errors mutating a [`Topology`] fails closed with, per
/// `agent/CODING_STANDARDS.md`'s "no stringly-typed errors" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyError {
    /// The topology already holds `N` CPU entries; no side effects occurred.
    Full,
}

/// Fixed-capacity, arch-neutral hardware topology: up to `N` CPU cores.
///
/// `N` is a caller-chosen capacity bound (analogous to `Pool<T, N>` in
/// `kernel::mem`), not a discovered value — a firmware source reporting more
/// cores than `N` fails closed via [`Topology::push`] returning
/// [`TopologyError::Full`] rather than growing unbounded storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Topology<const N: usize> {
    cpus: [Option<CpuDescriptor>; N],
    count: usize,
}

impl<const N: usize> Topology<N> {
    /// Creates an empty topology. `const fn`: no heap allocation, usable in
    /// a `static` initializer.
    pub const fn new() -> Self {
        Topology { cpus: [None; N], count: 0 }
    }

    /// Appends a discovered CPU core.
    ///
    /// Fails closed with [`TopologyError::Full`] and no side effects once
    /// `N` entries are already stored — never panics.
    pub fn push(&mut self, cpu: CpuDescriptor) -> Result<(), TopologyError> {
        if self.count >= N {
            return Err(TopologyError::Full);
        }
        self.cpus[self.count] = Some(cpu);
        self.count += 1;
        Ok(())
    }

    /// The number of CPU cores currently stored.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Whether no CPU cores have been stored yet.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Iterates over the stored CPU cores in discovery order.
    pub fn iter(&self) -> impl Iterator<Item = &CpuDescriptor> {
        self.cpus[..self.count].iter().filter_map(Option::as_ref)
    }
}

impl<const N: usize> Default for Topology<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu(id: u8) -> CpuDescriptor {
        CpuDescriptor { processor_id: id, interrupt_controller_id: id, enabled: true }
    }

    #[test]
    fn new_topology_is_empty() {
        let topology: Topology<4> = Topology::new();
        assert!(topology.is_empty());
        assert_eq!(topology.len(), 0);
        assert_eq!(topology.iter().count(), 0);
    }

    #[test]
    fn pushed_cpus_are_iterated_in_order() {
        let mut topology: Topology<4> = Topology::new();
        topology.push(cpu(0)).unwrap();
        topology.push(cpu(1)).unwrap();
        let ids: std::vec::Vec<u8> = topology.iter().map(|c| c.processor_id).collect();
        assert_eq!(ids, [0, 1]);
    }

    #[test]
    fn pushing_past_capacity_fails_closed_without_side_effects() {
        let mut topology: Topology<2> = Topology::new();
        topology.push(cpu(0)).unwrap();
        topology.push(cpu(1)).unwrap();

        assert_eq!(topology.push(cpu(2)), Err(TopologyError::Full));
        // Repeated overflow fails the same way every time, not just once.
        assert_eq!(topology.push(cpu(3)), Err(TopologyError::Full));
        assert_eq!(topology.len(), 2);
        let ids: std::vec::Vec<u8> = topology.iter().map(|c| c.processor_id).collect();
        assert_eq!(ids, [0, 1]);
    }

    #[test]
    fn disabled_cpu_entries_are_kept_not_dropped() {
        let mut topology: Topology<2> = Topology::new();
        topology
            .push(CpuDescriptor { processor_id: 0, interrupt_controller_id: 0, enabled: false })
            .unwrap();
        assert_eq!(topology.len(), 1);
        assert!(!topology.iter().next().unwrap().enabled);
    }
}
