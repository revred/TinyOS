//! TINYCMD — the canonical verb core and its DOS front-end, Phase 2's first slice.
//!
//! Layering (`EPIC-P2` §3.2, and the owner's note that the kernel beneath must serve
//! DOS, Linux/POSIX, RT *and* a minimal GUI shell equally): everything below the syntax
//! front-ends is flavour-agnostic. [`verbs`] executes typed requests against a
//! [`volume::RamVolume`] through the deny-by-default [`policy`] seam; [`dos`] is one thin
//! parser over that core and the POSIX/RT front-ends are later peers, not forks.
//!
//! The library is `no_std`, heap-free and `#![forbid(unsafe_code)]`; all output goes
//! through a `core::fmt::Write` sink so a QEMU serial fixture, a host test and a future
//! tab host render byte-identically — the property the golden-transcript parity gate
//! (`TEST-P2-07-01-A`) stands on.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod batch;
pub mod dos;
pub mod parity;
pub mod verbs;
pub mod volume;

/// Host-side test compilation of the spoor-journaling policy decorator
/// (`LE-56`). The decorator lives beside the fixture binary in
/// `spoor_policy.rs` and is **not** part of this library's shipped code —
/// this `#[cfg(test)]`-only include exists so its unit tests run under
/// `cargo test -p shell --lib` (the exact surface CI and the parity tab
/// drive), while the library itself stays kernel-free.
#[cfg(test)]
pub(crate) mod spoor_policy_host {
    use crate::policy::{GrantSet, VerbPolicy};
    use crate::verbs::{SpoorRow, SpoorView, VerbKind};

    /// The decorator, compiled here for its host-side tests — and for the
    /// parity harness's own tests, which install it to mirror the fixture.
    #[path = "../spoor_policy.rs"]
    pub mod spoor_policy;
}

/// Fixed capacities — the single reviewable location, per the capacities doctrine.
pub mod capacities {
    /// Maximum files the RAM volume holds.
    pub const MAX_FILES: usize = 24;
    /// Maximum directories (index 0 is the root).
    pub const MAX_DIRS: usize = 8;
    /// Maximum bytes per file.
    pub const MAX_DATA: usize = 512;
    /// Maximum bytes in one name component.
    pub const MAX_NAME: usize = 12;
    /// Maximum bytes in a rendered path.
    pub const MAX_PATH: usize = 64;
    /// Maximum environment variables per session.
    pub const MAX_ENV: usize = 8;
    /// Maximum bytes in an environment key.
    pub const MAX_ENV_KEY: usize = 12;
    /// Maximum bytes in an environment value.
    pub const MAX_ENV_VAL: usize = 48;
    /// Maximum bytes in one command line.
    pub const MAX_LINE: usize = 128;
    /// Maximum lines in one `.TCB` batch.
    pub const MAX_BATCH_LINES: usize = 64;
    /// Maximum lines `sort-stream` accepts.
    pub const MAX_SORT_LINES: usize = 16;
}

/// `G-SEC-5` labels, carried by every file from creation (`FEAT-P2-02`).
pub mod labels {
    /// Where the bytes came from.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Origin {
        /// Created by a verb in this session.
        Local,
        /// Seeded by the fixture/boot image.
        Seeded,
        /// Arrived from outside the machine.
        External,
    }
    /// Signature state.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Signer {
        /// No signature.
        Unsigned,
        /// Signed by the project key.
        ProjectKey,
    }
    /// Trust tier.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Trust {
        /// Untrusted content.
        Untrusted,
        /// Operator-trusted.
        Operator,
        /// System-trusted.
        System,
    }
    /// Derivation history bits — a transform records itself, never erases.
    pub const DERIVED_COPIED: u8 = 1;
    /// Set when the file has been renamed/moved.
    pub const DERIVED_RENAMED: u8 = 2;

    /// The label set. `quarantine` is sticky: no verb path clears it (`BND-13`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Labels {
        /// Origin of the bytes.
        pub origin: Origin,
        /// Signature state.
        pub signer: Signer,
        /// Trust tier.
        pub trust: Trust,
        /// Read-only entitlement (renders as DOS `R`).
        pub read_only: bool,
        /// Quarantined content: execution/consumption refused.
        pub quarantine: bool,
        /// Derivation bits (`DERIVED_*`).
        pub derivation: u8,
    }

    impl Labels {
        /// A seeded, operator-trusted, unquarantined label.
        pub const fn seeded() -> Self {
            Labels {
                origin: Origin::Seeded,
                signer: Signer::Unsigned,
                trust: Trust::Operator,
                read_only: false,
                quarantine: false,
                derivation: 0,
            }
        }
    }
}

/// The ACI seam: deny-by-default verb authorisation (`FEAT-P2-01`).
pub mod policy {
    use crate::verbs::VerbKind;

    /// The single decision point every request passes. No installed policy → nothing runs.
    pub trait VerbPolicy {
        /// May `session` execute `verb`? Identity is the session, never the payload.
        fn allows(&self, session: &str, verb: VerbKind) -> bool;
        /// Whether the session holds `supervisor` scope (RT-critical `task-kill`).
        fn supervisor(&self, _session: &str) -> bool {
            false
        }
    }

    /// The default: everything denied.
    pub struct DenyAll;
    impl VerbPolicy for DenyAll {
        fn allows(&self, _session: &str, _verb: VerbKind) -> bool {
            false
        }
    }

    /// A fixed grant set over verb kinds, with one optional withheld verb —
    /// the fixture uses the withheld slot to prove deny-inside-batch.
    pub struct GrantSet {
        /// Verbs granted.
        pub granted: &'static [VerbKind],
        /// One verb deliberately withheld even if listed.
        pub withheld: Option<VerbKind>,
        /// Supervisor scope.
        pub supervisor: bool,
    }
    impl VerbPolicy for GrantSet {
        fn allows(&self, _session: &str, verb: VerbKind) -> bool {
            if self.withheld == Some(verb) {
                return false;
            }
            self.granted.contains(&verb)
        }
        fn supervisor(&self, _session: &str) -> bool {
            self.supervisor
        }
    }
}

/// Rendering untrusted text inert (`EPIC-P2` §6.5 rule 3): C0 control bytes, `ESC` and
/// DEL are replaced with `?` before display, so a filename cannot move the cursor or
/// repaint the trusted region. Trusted shell output (e.g. `CLS`'s own `ESC[2J`) does not
/// pass through this — the rule is about *attacker-influenced* strings.
pub mod render {
    use core::fmt;

    /// Write `text` to `sink` with control bytes neutralised.
    pub fn write_inert(sink: &mut dyn fmt::Write, text: &str) -> fmt::Result {
        for ch in text.chars() {
            if ch < ' ' || ch == '\u{7f}' {
                sink.write_char('?')?;
            } else {
                sink.write_char(ch)?;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// A filename carrying `ESC[2J` renders inert (STORY-P2-01-01 acceptance 4).
        #[test]
        fn escape_sequences_are_neutralised() {
            let mut out = String::new();
            write_inert(&mut out, "EVIL\u{1b}[2J\rNAME").unwrap();
            assert_eq!(out, "EVIL?[2J?NAME");
        }
    }
}
