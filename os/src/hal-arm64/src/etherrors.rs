//! The discovery refusal taxonomy (`STORY-P1-09-07`/`-11`): which confession
//! code each refused rung earns, and which sixteen decisive bits it spells.
//!
//! Split from `ethernet.rs` so the pipeline reads as a pipeline: nothing
//! here decides an outcome — [`crate::ethernet::Discovery`] is the input,
//! and the two mappings are the whole module. How a code and its detail are
//! *pronounced* (blinks, sentences, canvas text) lives one seam further in
//! [`crate::ethernostics`].

use crate::ethernet::Discovery;
use crate::gem;
use crate::pcie::LinkAbsent;
use crate::rp1_clocks::ClockRefused;

/// `STORY-P1-09-07`: the first refused rung of discovery as a blink count —
/// the confession the proven lamp can carry when serial is dead and the
/// screen is dark. `None` is health: a known PHY keeps the plain pulse,
/// whatever its link state, so the lamp's ordinary language is undiluted.
///
/// Matched exhaustively on purpose (`TEST-P1-09-07-A` clause 1): a future
/// `Discovery` arm fails to compile here rather than silently sharing a code.
pub const fn blink_code(discovery: &Discovery) -> Option<u8> {
    match discovery {
        Discovery::LinkAbsent(absent) => Some(match absent {
            LinkAbsent::PortNotRc(_) => 1,
            LinkAbsent::PhyDown(_) => 2,
            LinkAbsent::LinkDown(_) => 3,
            LinkAbsent::WindowBase(_) => 4,
            LinkAbsent::WindowPci(_) => 5,
            LinkAbsent::WindowSpan(_) => 6,
            LinkAbsent::RootVendor(_) => 14,
            LinkAbsent::EndpointVendor(_) => 15,
            LinkAbsent::BarSilent(_) => 19,
            LinkAbsent::BarNotHeld(_) => 20,
            LinkAbsent::InboundNotHeld(_) => 21,
            LinkAbsent::InboundRemapNotHeld(_) => 22,
        }),
        Discovery::ClockRefused(refused) => Some(match refused {
            ClockRefused::BlockSilent { .. } => 16,
            ClockRefused::EnableNotHeld { .. } => 17,
            ClockRefused::NeverRan { .. } => 18,
        }),
        Discovery::IdentityRefused(refused) => Some(match refused {
            gem::IdentityError::FloatingBus => 7,
            gem::IdentityError::AllZeros => 8,
            gem::IdentityError::WrongModule(_) => 9,
        }),
        Discovery::Present { phy, .. } => match phy {
            gem::PhyOutcome::ReleaseStuck => Some(10),
            gem::PhyOutcome::Absent => Some(11),
            gem::PhyOutcome::PortWedged => Some(12),
            gem::PhyOutcome::Unknown { .. } => Some(13),
            gem::PhyOutcome::Known { .. } => None,
        },
    }
}

/// `STORY-P1-09-11`: the sixteen decisive bits each refusal spells after
/// its code — the wrong module itself, a vendor or status word's low half,
/// a window address in whole megabytes. Health never spells; the caller
/// only asks after [`blink_code`] said there is a refusal.
pub const fn blink_detail(discovery: &Discovery) -> u16 {
    match discovery {
        Discovery::LinkAbsent(absent) => match absent {
            LinkAbsent::PortNotRc(word)
            | LinkAbsent::PhyDown(word)
            | LinkAbsent::LinkDown(word)
            | LinkAbsent::RootVendor(word)
            | LinkAbsent::EndpointVendor(word) => *word as u16,
            LinkAbsent::WindowBase(value)
            | LinkAbsent::WindowPci(value)
            | LinkAbsent::WindowSpan(value) => (*value >> 20) as u16,
            // The masks and addresses that convict a BAR live in the high
            // half (0xFFC0_0000's mask, 0x0041_0000's address).
            LinkAbsent::BarSilent(word) | LinkAbsent::BarNotHeld(word) => (*word >> 16) as u16,
            // An inbound dword's decisive bits live in the low half — the
            // size code (0x15, 0xF01C) and the remap's ACCESS_EN bit.
            LinkAbsent::InboundNotHeld(word) | LinkAbsent::InboundRemapNotHeld(word) => {
                *word as u16
            }
        },
        Discovery::ClockRefused(refused) => match refused {
            // The halves that convict: poison spells its 0xDEAD high half
            // (57005); a dropped enable spells the low half where the
            // enable bit should be; a stopped clock spells the status half.
            ClockRefused::BlockSilent { sel } => (*sel >> 16) as u16,
            ClockRefused::EnableNotHeld { ctrl } => *ctrl as u16,
            ClockRefused::NeverRan { ctrl } => (*ctrl >> 16) as u16,
        },
        Discovery::IdentityRefused(refused) => match refused {
            gem::IdentityError::FloatingBus => 0xFFFF,
            gem::IdentityError::AllZeros => 0,
            gem::IdentityError::WrongModule(module) => *module,
        },
        Discovery::Present { phy, .. } => match phy {
            gem::PhyOutcome::Unknown { id1, .. } => *id1,
            _ => 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gem::{IdentityError, LinkState, PhyOutcome, Speed};

    // TEST-P1-09-07-A clause 1: the mapping is total, distinct, refusal-only.

    #[test]
    fn every_refusal_earns_a_distinct_code_and_health_earns_none() {
        let refusals: Vec<Discovery> = vec![
            Discovery::LinkAbsent(LinkAbsent::PortNotRc(0)),
            Discovery::LinkAbsent(LinkAbsent::PhyDown(0x80)),
            Discovery::LinkAbsent(LinkAbsent::LinkDown(0x90)),
            Discovery::LinkAbsent(LinkAbsent::WindowBase(0)),
            Discovery::LinkAbsent(LinkAbsent::WindowPci(1)),
            Discovery::LinkAbsent(LinkAbsent::WindowSpan(2)),
            Discovery::LinkAbsent(LinkAbsent::RootVendor(0xFFFF_FFFF)),
            Discovery::LinkAbsent(LinkAbsent::EndpointVendor(0)),
            Discovery::LinkAbsent(LinkAbsent::BarSilent(0xFFFF_FFF0)),
            Discovery::LinkAbsent(LinkAbsent::BarNotHeld(0xFFC0_0000)),
            Discovery::LinkAbsent(LinkAbsent::InboundNotHeld(0xDEAD_DEAD)),
            Discovery::LinkAbsent(LinkAbsent::InboundRemapNotHeld(0)),
            Discovery::ClockRefused(ClockRefused::BlockSilent { sel: 0xDEAD_0000 }),
            Discovery::ClockRefused(ClockRefused::EnableNotHeld { ctrl: 0 }),
            Discovery::ClockRefused(ClockRefused::NeverRan { ctrl: 0x800 }),
            Discovery::IdentityRefused(IdentityError::FloatingBus),
            Discovery::IdentityRefused(IdentityError::AllZeros),
            Discovery::IdentityRefused(IdentityError::WrongModule(2)),
            Discovery::Present { revision: 1, phy: PhyOutcome::ReleaseStuck, link: None },
            Discovery::Present { revision: 1, phy: PhyOutcome::Absent, link: None },
            Discovery::Present { revision: 1, phy: PhyOutcome::PortWedged, link: None },
            Discovery::Present {
                revision: 1,
                phy: PhyOutcome::Unknown { address: 0, id1: 1, id2: 2 },
                link: None,
            },
        ];
        let codes: Vec<u8> =
            refusals.iter().map(|d| blink_code(d).expect("every refusal speaks")).collect();
        let mut deduped = codes.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(deduped.len(), codes.len(), "no two refusals may share a code: {codes:?}");
        assert!(codes.iter().all(|&code| code > 0));
        // The first rung of each family, pinned by number so a session log
        // can be read years later without the source.
        assert_eq!(blink_code(&refusals[0]), Some(1));
        assert_eq!(blink_code(&refusals[6]), Some(14), "root-vendor counts 14");
        assert_eq!(blink_code(&refusals[7]), Some(15), "endpoint-vendor counts 15");
        assert_eq!(blink_code(&refusals[8]), Some(19), "bar-silent counts 19");
        assert_eq!(blink_code(&refusals[9]), Some(20), "bar-held counts 20");
        assert_eq!(blink_code(&refusals[10]), Some(21), "ibw-held counts 21");
        assert_eq!(blink_code(&refusals[11]), Some(22), "ibw-remap counts 22");
        assert_eq!(blink_code(&refusals[12]), Some(16), "clk-silent counts 16");
        assert_eq!(blink_code(&refusals[13]), Some(17), "clk-enable counts 17");
        assert_eq!(blink_code(&refusals[14]), Some(18), "clk-stuck counts 18");
        assert_eq!(blink_code(&refusals[15]), Some(7));
        assert_eq!(blink_code(&refusals[18]), Some(10));
        // Health — a known PHY in any link state — keeps the plain pulse.
        for link in [
            None,
            Some(LinkState::Down),
            Some(LinkState::Unresolved),
            Some(LinkState::Up { speed: Speed::Mbps1000, full_duplex: true }),
        ] {
            let healthy = Discovery::Present {
                revision: 0x0109,
                phy: PhyOutcome::Known { address: 1, id1: 0x600D, id2: 0x84A2 },
                link,
            };
            assert_eq!(blink_code(&healthy), None, "health never blinks a code: {link:?}");
        }
    }

    // TEST-P1-09-11-A clause 3: detail selection is total, arm by arm.

    #[test]
    fn every_refusal_selects_its_named_sixteen_bits() {
        assert_eq!(blink_detail(&Discovery::IdentityRefused(IdentityError::WrongModule(2))), 2);
        assert_eq!(blink_detail(&Discovery::IdentityRefused(IdentityError::FloatingBus)), 0xFFFF);
        assert_eq!(blink_detail(&Discovery::IdentityRefused(IdentityError::AllZeros)), 0);
        assert_eq!(blink_detail(&Discovery::LinkAbsent(LinkAbsent::LinkDown(0x90))), 0x90);
        assert_eq!(
            blink_detail(&Discovery::LinkAbsent(LinkAbsent::RootVendor(0x2712_14E4))),
            0x14E4,
            "a vendor dword spells its vendor half"
        );
        // Window addresses spell the low sixteen bits of their megabyte
        // index — 0x1E_0000_0000 is MiB 0x1E000, truncating to 0xE000.
        assert_eq!(
            blink_detail(&Discovery::LinkAbsent(LinkAbsent::WindowBase(0x0000_001E_0000_0000))),
            0xE000
        );
        assert_eq!(
            blink_detail(&Discovery::LinkAbsent(LinkAbsent::BarSilent(0xFFFF_FFF0))),
            0xFFFF,
            "a floating probe spells its high half"
        );
        assert_eq!(
            blink_detail(&Discovery::LinkAbsent(LinkAbsent::BarNotHeld(0xFFC0_0000))),
            0xFFC0,
            "a dropped assignment spells the readback's high half"
        );
        assert_eq!(
            blink_detail(&Discovery::LinkAbsent(LinkAbsent::InboundNotHeld(0xABCD_F01C))),
            0xF01C,
            "an inbound dword spells its low half — where the size code lives"
        );
        assert_eq!(
            blink_detail(&Discovery::LinkAbsent(LinkAbsent::InboundRemapNotHeld(0x0013_0000))),
            0x0000,
            "a remap spells its low half — where ACCESS_EN should be"
        );
        assert_eq!(
            blink_detail(&Discovery::ClockRefused(ClockRefused::BlockSilent { sel: 0xDEAD_0000 })),
            0xDEAD,
            "fabric poison spells 57005"
        );
        assert_eq!(
            blink_detail(&Discovery::ClockRefused(ClockRefused::EnableNotHeld {
                ctrl: 0x0000_0400
            })),
            0x0400,
            "a dropped enable spells the low half"
        );
        assert_eq!(
            blink_detail(&Discovery::ClockRefused(ClockRefused::NeverRan { ctrl: 0x0000_0800 })),
            0,
            "a stopped clock spells the status half"
        );
        assert_eq!(
            blink_detail(&Discovery::Present {
                revision: 1,
                phy: PhyOutcome::Unknown { address: 0, id1: 0x0141, id2: 0x0C86 },
                link: None,
            }),
            0x0141,
            "an unknown PHY spells its ID1"
        );
    }
}
