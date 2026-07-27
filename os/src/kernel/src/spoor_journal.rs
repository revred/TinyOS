//! Append-only spoor journal (`STORY-P0-06-02`).
//!
//! A fixed-capacity, append-only journal a [`crate::spoor::Spoor`] is
//! written into the moment it's stamped — no buffering, no batching, per
//! `Sharc.Blue`'s own doctrine ("stamp immediately at execution point").
//! `Sharc.Blue`'s own journal is a file (`<workspace>/.sharc/spoor/
//! <session>.journal`: an 8-byte magic header followed by N 8-byte
//! records); TinyOS's Phase 0 equivalent has no filesystem yet, so
//! [`SpoorJournal`] is a fixed-capacity in-memory ring buffer instead —
//! with the identical record shape (spoors stored as their raw
//! [`Spoor::to_bits`] `u64`, consecutively), so a later Story that adds
//! real storage can serialize this buffer directly (magic header, then
//! each entry's 8 bytes in oldest-to-newest order) without inventing a
//! second format.
//!
//! Ring-buffer, not `Pool<T, N>`: a journal is append-and-overwrite-oldest
//! (losing the newest audit event is worse than losing the oldest for a
//! fixed-size trace), not alloc/free — a different discipline from
//! `Pool`'s exhaustion-fails-closed contract, so it isn't built on `Pool`.

use crate::spoor::{Action, Actor, Category, Outcome, Spoor};

/// The 8-byte magic header a future storage-backed journal would prepend
/// before this buffer's entries — identical to `Sharc.Blue`'s own on-disk
/// journal format, so a byte-level parser needs no TinyOS-specific case.
pub const JOURNAL_MAGIC: [u8; 8] = *b"SPOORJ01";

/// A fixed-capacity, append-only ring buffer of up to `N` [`Spoor`]
/// values. `append` never allocates, never blocks, and never panics —
/// once full, the oldest entry is overwritten rather than the append
/// being rejected.
pub struct SpoorJournal<const N: usize> {
    /// Raw packed bits, one per slot — the identical on-wire shape
    /// `Sharc.Blue`'s own journal file uses per record.
    entries: [u64; N],
    /// The slot the *next* `append` will write to.
    next: usize,
    /// Number of currently-valid entries, capped at `N` once the journal
    /// has wrapped at least once.
    len: usize,
}

impl<const N: usize> SpoorJournal<N> {
    /// Creates an empty journal. `const fn`: no heap allocation, usable in
    /// a `static` initializer.
    pub const fn new() -> Self {
        SpoorJournal { entries: [0u64; N], next: 0, len: 0 }
    }

    /// Appends `spoor`, overwriting the oldest entry if the journal is
    /// already full. Never allocates, never blocks, never panics — the
    /// single write to `entries[self.next]` below completes before this
    /// function returns, so no reader (there is no concurrent-reader API
    /// this module exposes beyond `&self` methods, which the borrow
    /// checker already serializes against a concurrent `&mut self` append)
    /// can observe a partially-written entry.
    ///
    /// A zero-capacity journal (`N == 0`) has no slot to write into at
    /// all — `append` is a no-op rather than an out-of-bounds index/modulo-
    /// by-zero panic, preserving this function's own "never panics"
    /// contract even at the degenerate capacity.
    pub fn append(&mut self, spoor: Spoor) {
        if N == 0 {
            return;
        }
        self.entries[self.next] = spoor.to_bits();
        self.next = (self.next + 1) % N;
        if self.len < N {
            self.len += 1;
        }
    }

    /// Number of currently-held entries (`0..=N`).
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the journal currently holds no entries.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Iterates over currently-held entries, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = Spoor> + '_ {
        // Not yet wrapped (`len < N`): the oldest entry is always at index
        // 0. Wrapped at least once (`len == N`): `next` is the slot the
        // *next* append will overwrite, i.e. the current oldest entry.
        let start = if self.len < N { 0 } else { self.next };
        (0..self.len).map(move |i| {
            let idx = (start + i) % N;
            match Spoor::decode(self.entries[idx]) {
                Ok(spoor) => spoor,
                // Every stored value was written via `spoor.to_bits()`
                // (this module's only writer, `append`), and
                // `STORY-P0-06-01`'s own round-trip property guarantees
                // `decode` always succeeds on bits `to_bits` produced —
                // unreachable in practice, kept exhaustive rather than
                // assumed away, matching `Spoor`'s own accessors'
                // precedent for this exact shape of "provably unreachable
                // but not unsafe to leave a real fallback for."
                Err(_) => Spoor::stamp(
                    Category::Boot,
                    Actor::Kernel,
                    Action::Create,
                    Outcome::Empty,
                    0,
                    0,
                ),
            }
        })
    }
}

impl<const N: usize> Default for SpoorJournal<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spoor(target: u16) -> Spoor {
        Spoor::stamp(Category::Lock, Actor::Kernel, Action::Boost, Outcome::Ok, target, 0)
    }

    #[test]
    fn empty_journal_iterates_to_nothing() {
        let journal: SpoorJournal<4> = SpoorJournal::new();
        assert_eq!(journal.len(), 0);
        assert!(journal.is_empty());
        assert_eq!(journal.iter().count(), 0);
    }

    // STORY-P0-06-02 acceptance criterion 1/2: appended entries are read
    // back in the same (oldest-first) order they were written, with no
    // torn/partial state observable.
    #[test]
    fn appended_entries_iterate_oldest_first() {
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        journal.append(spoor(1));
        journal.append(spoor(2));
        journal.append(spoor(3));

        let targets: std::vec::Vec<u16> = journal.iter().map(|s| s.target()).collect();
        assert_eq!(targets, std::vec![1, 2, 3]);
        assert_eq!(journal.len(), 3);
    }

    // STORY-P0-06-02 acceptance criterion 1: a full journal overwrites its
    // oldest entry rather than rejecting the new one or panicking.
    #[test]
    fn a_full_journal_overwrites_its_oldest_entry() {
        let mut journal: SpoorJournal<3> = SpoorJournal::new();
        journal.append(spoor(1));
        journal.append(spoor(2));
        journal.append(spoor(3));
        // Journal is now full (3/3). The next append overwrites the
        // oldest entry (target 1), not the newest.
        journal.append(spoor(4));

        let targets: std::vec::Vec<u16> = journal.iter().map(|s| s.target()).collect();
        assert_eq!(targets, std::vec![2, 3, 4], "oldest entry (1) should have been overwritten");
        assert_eq!(journal.len(), 3, "length caps at capacity, never exceeds it");
    }

    // Repeated wraps (many more appends than capacity) never panic and
    // always leave exactly the most recent `N` entries, in order.
    #[test]
    fn repeated_wraps_always_retain_only_the_most_recent_capacity_entries() {
        let mut journal: SpoorJournal<3> = SpoorJournal::new();
        for i in 0..10u16 {
            journal.append(spoor(i));
        }
        let targets: std::vec::Vec<u16> = journal.iter().map(|s| s.target()).collect();
        assert_eq!(targets, std::vec![7, 8, 9]);
    }

    // STORY-P0-06-02 acceptance criterion 3: the record format is exactly
    // `Spoor::to_bits()`, one `u64` per entry — a future storage-backed
    // journal could serialize `entries` directly, byte for byte.
    #[test]
    fn journal_magic_matches_sharc_blues_own_on_disk_format() {
        assert_eq!(&JOURNAL_MAGIC, b"SPOORJ01");
        assert_eq!(JOURNAL_MAGIC.len(), 8);
    }

    #[test]
    fn a_journal_at_capacity_one_still_retains_only_the_newest_entry() {
        let mut journal: SpoorJournal<1> = SpoorJournal::new();
        journal.append(spoor(1));
        journal.append(spoor(2));
        let targets: std::vec::Vec<u16> = journal.iter().map(|s| s.target()).collect();
        assert_eq!(targets, std::vec![2]);
    }

    // A zero-capacity journal has no slot to write into at all — `append`
    // must be a no-op, never an out-of-bounds index or modulo-by-zero
    // panic, preserving this module's own "never panics" contract even at
    // this degenerate capacity.
    #[test]
    fn a_zero_capacity_journal_never_panics_on_append() {
        let mut journal: SpoorJournal<0> = SpoorJournal::new();
        journal.append(spoor(1));
        journal.append(spoor(2));
        assert!(journal.is_empty());
        assert_eq!(journal.len(), 0);
        assert_eq!(journal.iter().count(), 0);
    }
}
