//! The Raspberry Pi 5 hardware run path (`STORY-P1-07-05`, `TEST-P1-07-05-A`).
//!
//! One command — `cargo run -p xtask -- pi5 --fixture=<name>` — builds the
//! bootable AArch64 binary, flattens it into the placeable `kernel8.img` the
//! firmware expects, prints exactly where the artifacts go on the boot
//! partition, captures the debug UART, and exits with the **same code scheme
//! as `qemu-x86_64`**. It is deliberately not a second harness: the verdict it
//! consumes is `STORY-P1-01-02`'s existing `TINYOS-RESULT/1` protocol, parsed
//! by the same [`crate::timing::parse_result`] the Tier 0 path trusts, because
//! a second, divergent harness is the shape `LE-06` already cost this project
//! once.
//!
//! Everything in this module except the two seam implementors at the bottom is
//! a pure function over captured text or bytes, host-unit-tested without a
//! board (`TEST-P1-07-05-A` clause 6, `SEC-19`). The captured bytes are
//! hostile input (`BND-03`, `PD-12`): parsing assumes no well-formedness and
//! no framing, capture size is bounded (`SEC-20`), and a partial or corrupt
//! verdict line is a *failure to read a verdict*, never a verdict.

use std::io::Read;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::timing;
use crate::XtaskExit;

/// Where the Raspberry Pi firmware places a flat image and jumps.
///
/// Mirrors `hal-arm64`'s `board::KERNEL_LOAD_ADDRESS` and the `.ld` script's
/// origin — those two are pinned together by a test in that crate; this
/// constant is validated against the *built ELF* by [`flatten_elf`], so a
/// drift between the linker script and this run path fails the build step
/// rather than producing an image that silently executes from its middle
/// (divergence record §3: the firmware needs `os_check=0` for this address to
/// hold at all).
pub const KERNEL_LOAD_ADDRESS: u64 = 0x8_0000;

/// Ceiling on the flattened image, so a linker-script mistake that scatters a
/// segment to a distant address fails as an error instead of a multi-gigabyte
/// zero-filled file.
pub const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;

/// The package and binary every `pi5` fixture builds. One crate on purpose:
/// the image is packaging around `hal-arm64`'s boot path, not a place for
/// board logic to accumulate.
pub const IMAGE_PACKAGE: &str = "pi5-image";
/// The `[[bin]]` name inside [`IMAGE_PACKAGE`].
pub const IMAGE_BINARY: &str = "pi5-image";

/// One Tier 1 hardware fixture the `pi5` subcommand accepts.
///
/// The same registration discipline as the Tier 0 [`crate::FIXTURES`] table
/// (`STORY-P0-01-04`: a fixture nobody can enumerate is an unverified fixture
/// that looks verified), in a separate namespace exactly as `measure`'s
/// already is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pi5Fixture {
    /// `--fixture=` value.
    pub name: &'static str,
    /// Cargo feature selecting this fixture's build, or `None` for the
    /// default boot image.
    pub feature: Option<&'static str>,
    /// The Test document that owns this fixture's pass condition.
    pub owning_test: &'static str,
    /// What a passing run demonstrates.
    pub summary: &'static str,
}

/// Every Pi 5 fixture. Manual runs only — CI stays Tier 0 per the recorded
/// `FEAT-P1-07` §7.4 decision (b).
pub const PI5_FIXTURES: &[Pi5Fixture] = &[Pi5Fixture {
    name: "boot",
    feature: None,
    owning_test: "TEST-P1-07-01-A",
    summary: "Boot to EL1: CurrentEL first, READY sequence, vectors installed, verdict on the UART",
}];

/// Resolves a `--fixture=` value against [`PI5_FIXTURES`].
pub fn pi5_fixture(name: &str) -> Option<&'static Pi5Fixture> {
    PI5_FIXTURES.iter().find(|fixture| fixture.name == name)
}

/// A flat, placeable image: the bytes `kernel8.img` will hold, plus what the
/// ELF said about itself so the run record can quote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatImage {
    /// The image bytes, gaps between load segments zero-filled.
    pub bytes: Vec<u8>,
    /// The ELF entry point, validated equal to [`KERNEL_LOAD_ADDRESS`].
    pub entry: u64,
    /// The lowest load segment's physical address, validated equal to
    /// [`KERNEL_LOAD_ADDRESS`].
    pub load_address: u64,
}

/// Flattens an ELF64 into the raw image the Pi firmware loads.
///
/// The workspace's own build output — parsed defensively anyway, because a
/// truncated or corrupted artifact must fail here with a reason, not become a
/// silent board. `objcopy -O binary`'s job, done in-process so the run path
/// has no external tool dependency and the entry-point-is-first-byte property
/// is *checked*, not hoped (an `ENTRY()` directive alone does not survive
/// flattening — divergence record, layout check).
pub fn flatten_elf(elf: &[u8]) -> Result<FlatImage, String> {
    let read_u16 = |at: usize| u16::from_le_bytes([elf[at], elf[at + 1]]);
    let read_u32 = |at: usize| u32::from_le_bytes([elf[at], elf[at + 1], elf[at + 2], elf[at + 3]]);
    let read_u64 = |at: usize| {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&elf[at..at + 8]);
        u64::from_le_bytes(bytes)
    };

    if elf.len() < 64 {
        return Err(format!("ELF header truncated: {} bytes", elf.len()));
    }
    if elf[0..4] != [0x7F, b'E', b'L', b'F'] {
        return Err("not an ELF: bad magic".to_string());
    }
    if elf[4] != 2 {
        return Err("not a 64-bit ELF".to_string());
    }
    if elf[5] != 1 {
        return Err("not a little-endian ELF".to_string());
    }
    let machine = read_u16(18);
    if machine != 183 {
        return Err(format!("e_machine {machine} is not AArch64 (183)"));
    }
    let entry = read_u64(24);
    let phoff = usize::try_from(read_u64(32)).map_err(|_| "e_phoff out of range".to_string())?;
    let phentsize = usize::from(read_u16(54));
    let phnum = usize::from(read_u16(56));
    if phentsize < 56 {
        return Err(format!("e_phentsize {phentsize} is smaller than an ELF64 program header"));
    }
    if phnum == 0 {
        return Err("no program headers".to_string());
    }
    // A bring-up image has a handful of segments; hundreds means the header
    // is garbage, and this bound keeps the loop over it bounded too.
    if phnum > 128 {
        return Err(format!("{phnum} program headers is not a plausible flat image"));
    }

    // (paddr, file offset, length) for every PT_LOAD that carries file bytes.
    let mut segments: Vec<(u64, usize, usize)> = Vec::new();
    for index in 0..phnum {
        let header = phoff
            .checked_add(index.checked_mul(phentsize).ok_or("program header overflow")?)
            .ok_or("program header overflow")?;
        let end = header.checked_add(56).ok_or("program header overflow")?;
        if end > elf.len() {
            return Err(format!("program header {index} runs past the end of the file"));
        }
        if read_u32(header) != 1 {
            continue; // not PT_LOAD
        }
        let offset = read_u64(header + 8);
        let paddr = read_u64(header + 24);
        let filesz = read_u64(header + 32);
        if filesz == 0 {
            continue; // NOBITS-only (e.g. .bss/.stack): the stub zeroes it
        }
        let data_start =
            usize::try_from(offset).map_err(|_| "segment offset out of range".to_string())?;
        let data_len =
            usize::try_from(filesz).map_err(|_| "segment size out of range".to_string())?;
        let data_end =
            data_start.checked_add(data_len).ok_or("segment bounds overflow".to_string())?;
        if data_end > elf.len() {
            return Err(format!(
                "segment {index} claims bytes {data_start}..{data_end} but the file has {}",
                elf.len()
            ));
        }
        if paddr.checked_add(filesz).is_none() {
            return Err(format!("segment {index} wraps the physical address space"));
        }
        segments.push((paddr, data_start, data_len));
    }
    if segments.is_empty() {
        return Err("no PT_LOAD segment carries file bytes".to_string());
    }

    let load_address =
        segments.iter().map(|(paddr, _, _)| *paddr).min().expect("segments is non-empty");
    if load_address != KERNEL_LOAD_ADDRESS {
        return Err(format!(
            "load address {load_address:#x} is not {KERNEL_LOAD_ADDRESS:#x}: the firmware \
             (with os_check=0) places kernel8.img at {KERNEL_LOAD_ADDRESS:#x} and nowhere else"
        ));
    }
    if entry != KERNEL_LOAD_ADDRESS {
        return Err(format!(
            "entry point {entry:#x} is not the load address {KERNEL_LOAD_ADDRESS:#x}: the \
             firmware jumps to the first byte of the file, and ENTRY() does not survive \
             flattening"
        ));
    }

    let image_end = segments
        .iter()
        .map(|(paddr, _, len)| paddr + *len as u64)
        .max()
        .expect("segments is non-empty");
    let image_size = usize::try_from(image_end - load_address)
        .map_err(|_| "image size out of range".to_string())?;
    if image_size > MAX_IMAGE_BYTES {
        return Err(format!(
            "flattened image would be {image_size} bytes (ceiling {MAX_IMAGE_BYTES}): a \
             segment is linked somewhere it cannot belong"
        ));
    }

    let mut bytes = vec![0u8; image_size];
    for (paddr, data_start, data_len) in &segments {
        let to = usize::try_from(paddr - load_address).expect("bounded by image_size");
        bytes[to..to + data_len].copy_from_slice(&elf[*data_start..data_start + data_len]);
    }
    Ok(FlatImage { bytes, entry, load_address })
}

/// SHA-256 of `bytes`, as lowercase hex.
///
/// Hand-rolled (FIPS 180-4) rather than a new dependency: `xtask` has zero
/// dependencies today and an integrity hash over a capture does not justify
/// the first one. Verified against the FIPS test vectors below.
pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let mut message = bytes.to_vec();
    let bit_length = (bytes.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());

    // The padding above makes the length an exact multiple of 64, so the
    // `as_chunks` remainders are empty by construction.
    for block in message.as_chunks::<64>().0 {
        let mut schedule = [0u32; 64];
        for (word, chunk) in schedule.iter_mut().zip(block.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*chunk);
        }
        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(schedule[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (word, add) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *word = word.wrapping_add(add);
        }
    }

    let mut hex = String::with_capacity(64);
    for word in state {
        hex.push_str(&format!("{word:08x}"));
    }
    hex
}

/// What one poll of the serial seam produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chunk {
    /// Bytes arrived.
    Bytes(Vec<u8>),
    /// Nothing arrived within the source's own poll interval.
    Idle,
    /// The source is gone (adapter unplugged, reader thread dead).
    Disconnected,
}

/// The serial seam: the only I/O in the run path, per `TEST-P1-07-05-A`
/// clause 6. The real implementor is [`ChannelChunks`]; tests script it.
pub trait SerialChunks {
    /// Waits up to the source's own poll interval for the next event.
    fn next_chunk(&mut self) -> Chunk;
}

/// The clock seam, so timeout behaviour is tested with a scripted clock
/// rather than with real sleeps.
pub trait Clock {
    /// Milliseconds since an arbitrary fixed origin.
    fn now_ms(&self) -> u64;
}

/// Capture bounds. All three exist because a bring-up board is hostile input:
/// one that never speaks must not hang the host, one that babbles must not
/// exhaust it (`SEC-20`), and one that spoke and stopped must be reported as
/// exactly that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapturePolicy {
    /// Hard cap on captured bytes (`SEC-20`).
    pub max_bytes: usize,
    /// Overall deadline for the whole capture.
    pub overall_ms: u64,
    /// After the first byte, how long a silence ends the capture.
    pub quiet_ms: u64,
}

impl CapturePolicy {
    /// The bring-up default: a manual power-cycle fits in the overall window,
    /// and the quiet window is long enough for the firmware's own pauses.
    pub const BRING_UP: CapturePolicy =
        CapturePolicy { max_bytes: 1024 * 1024, overall_ms: 90_000, quiet_ms: 10_000 };
}

/// Why a capture ended. Recorded in the run record verbatim: the *reason the
/// tool stopped listening* is part of what makes a quoted capture evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureEnd {
    /// A complete `TINYOS-RESULT/1` line arrived — nothing more is coming
    /// that could change the verdict.
    VerdictSeen,
    /// The overall deadline passed.
    OverallTimeout,
    /// Bytes arrived, then the quiet window elapsed with no more.
    QuietAfterBytes,
    /// The byte cap was reached (`SEC-20`): a board stuck in an output loop.
    ByteCapReached,
    /// The serial source went away mid-capture.
    Disconnected,
}

impl CaptureEnd {
    /// The name written into the run record.
    pub fn as_str(self) -> &'static str {
        match self {
            CaptureEnd::VerdictSeen => "verdict-seen",
            CaptureEnd::OverallTimeout => "overall-timeout",
            CaptureEnd::QuietAfterBytes => "quiet-after-bytes",
            CaptureEnd::ByteCapReached => "byte-cap-reached",
            CaptureEnd::Disconnected => "disconnected",
        }
    }
}

/// Drains the serial seam under `policy`, returning what arrived and why the
/// capture stopped.
pub fn capture<S: SerialChunks, C: Clock>(
    source: &mut S,
    clock: &C,
    policy: &CapturePolicy,
) -> (Vec<u8>, CaptureEnd) {
    let started = clock.now_ms();
    let mut bytes: Vec<u8> = Vec::new();
    let mut last_byte_at: Option<u64> = None;

    loop {
        match source.next_chunk() {
            Chunk::Bytes(chunk) => {
                let room = policy.max_bytes.saturating_sub(bytes.len());
                bytes.extend_from_slice(&chunk[..chunk.len().min(room)]);
                last_byte_at = Some(clock.now_ms());
                // Verdict before cap: a verdict that arrived in the same
                // chunk that filled the buffer is still a verdict.
                if contains_complete_verdict_line(&bytes) {
                    return (bytes, CaptureEnd::VerdictSeen);
                }
                if bytes.len() >= policy.max_bytes {
                    return (bytes, CaptureEnd::ByteCapReached);
                }
            }
            Chunk::Idle => {}
            Chunk::Disconnected => return (bytes, CaptureEnd::Disconnected),
        }

        let now = clock.now_ms();
        if now.saturating_sub(started) >= policy.overall_ms {
            return (bytes, CaptureEnd::OverallTimeout);
        }
        // The quiet window starts at the first byte, deliberately: before it,
        // the operator may still be power-cycling, and silence must get the
        // whole overall window rather than being cut short.
        if let Some(last) = last_byte_at {
            if now.saturating_sub(last) >= policy.quiet_ms {
                return (bytes, CaptureEnd::QuietAfterBytes);
            }
        }
    }
}

/// The four distinguishable outcomes of a hardware run — `TEST-P1-07-05-A`
/// clause 3's whole point. Silence, truncation and failure are three
/// different exits, because on this hardware during bring-up silence is the
/// *common* case, and a tool that reports it as "still working" or as
/// "failed" sends the next hour to the wrong hypothesis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pi5Outcome {
    /// A well-formed verdict said `ok=true`.
    Pass {
        /// The fixture name the board itself reported.
        fixture: String,
    },
    /// A well-formed verdict said `ok=false`.
    ReportedFailure {
        /// The fixture name the board itself reported.
        fixture: String,
    },
    /// Not one byte arrived.
    Silence,
    /// Bytes arrived but no trustworthy verdict did — a board that spoke and
    /// stopped, or a partial/corrupt/ambiguous verdict line, which is a
    /// failure to read a verdict, never a verdict.
    SpokeWithoutVerdict {
        /// Why no verdict could be read.
        detail: String,
    },
}

/// Classifies a completed capture, failing closed.
pub fn classify(captured: &[u8]) -> Pi5Outcome {
    if captured.is_empty() {
        return Pi5Outcome::Silence;
    }
    let text = String::from_utf8_lossy(captured);
    match timing::parse_result(&text) {
        Ok(result) if result.ok => Pi5Outcome::Pass { fixture: result.fixture },
        Ok(result) => Pi5Outcome::ReportedFailure { fixture: result.fixture },
        Err(error) => Pi5Outcome::SpokeWithoutVerdict { detail: error.to_string() },
    }
}

/// Maps an outcome onto the process exit scheme — the Tier 0 scheme, extended.
///
/// `0`/`1`/`2` keep exactly their `qemu-x86_64` meanings (pass, the thing
/// under test failed, harness error), so a reader who trusts the Tier 0 path
/// reads this one without learning anything new; `3` and `4` are the two
/// outcomes only hardware can produce.
pub fn outcome_exit(outcome: &Pi5Outcome) -> XtaskExit {
    match outcome {
        Pi5Outcome::Pass { .. } => XtaskExit::KernelBootSucceeded,
        Pi5Outcome::ReportedFailure { .. } => XtaskExit::KernelBootFailed,
        Pi5Outcome::Silence => XtaskExit::BoardSilent,
        Pi5Outcome::SpokeWithoutVerdict { .. } => XtaskExit::BoardSpokeWithoutVerdict,
    }
}

/// True once the buffer holds a complete (newline-terminated) result line —
/// the capture loop's early-out, so a verdict already in hand does not wait
/// out the quiet window.
pub fn contains_complete_verdict_line(bytes: &[u8]) -> bool {
    let sentinel = timing::RESULT_SENTINEL.as_bytes();
    // Any newline after the first sentinel occurrence terminates *some* line
    // containing it; whether that line is well formed is `classify`'s job,
    // and a malformed one ends the capture just as decisively.
    match bytes.windows(sentinel.len()).position(|window| window == sentinel) {
        Some(at) => bytes[at + sentinel.len()..].contains(&b'\n'),
        None => false,
    }
}

/// The SD-card placement instructions, printed after every build so nothing
/// about the image is folklore held by whoever did it last (`TEST-P1-07-05-A`
/// clause 1). Every line traces to the divergence record
/// (`session/hand-2026-07-28/23-bcm2712-divergence-record.md`).
pub fn placement_instructions(image_bytes: usize, image_sha256: &str) -> String {
    format!(
        "place on the SD card's boot (FAT32) partition:\n\
         \x20 kernel8.img   {image_bytes} bytes, sha256 {image_sha256}\n\
         \x20 config.txt    must contain BOTH of these lines:\n\
         \x20                 os_check=0\n\
         \x20                 kernel=kernel8.img\n\
         \x20   (without os_check=0 the Pi 5 firmware relocates the image to 0x200000 and\n\
         \x20    execution starts mid-image, as total silence — divergence record §3, the\n\
         \x20    one constant no test can check because config.txt lives on the card)\n\
         serial: the dedicated 3-pin debug connector (NOT the GPIO header), 115200 8N1;\n\
         \x20       loopback-test the adapter first (TEST-P1-07-01-A clause 1)\n\
         then: reinsert the card and power-cycle the board\n"
    )
}

/// Everything a Report needs to trace a quoted capture back to the invocation
/// that produced it (`TEST-P1-07-05-A` clause 7, `SEC-14`, `BND-17`).
///
/// `board_revision` and `firmware_version` are operator-supplied (the board
/// cannot report them before it boots); "unrecorded" is written rather than
/// omitting the field, per the honest-absence rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRecord<'a> {
    /// `git rev-parse HEAD` at invocation, or "unrecorded".
    pub commit: &'a str,
    /// The `--fixture=` value.
    pub fixture: &'a str,
    /// The serial device the capture came from.
    pub port: &'a str,
    /// The configured baud rate.
    pub baud: u32,
    /// Operator-supplied board revision, or "unrecorded".
    pub board_revision: &'a str,
    /// Operator-supplied firmware version, or "unrecorded".
    pub firmware_version: &'a str,
    /// SHA-256 of the flat image that was placed.
    pub image_sha256: &'a str,
    /// Size of the flat image in bytes.
    pub image_bytes: usize,
    /// SHA-256 of the raw capture.
    pub capture_sha256: &'a str,
    /// Size of the raw capture in bytes.
    pub capture_bytes: usize,
    /// Why the capture stopped.
    pub capture_end: CaptureEnd,
    /// The classified outcome.
    pub outcome: &'a Pi5Outcome,
    /// Seconds since the Unix epoch at invocation.
    pub timestamp_unix: u64,
}

/// Renders the run record as `key<TAB>value` lines, one fact per line, in a
/// fixed order — the same shape as every other machine-read register in this
/// repository.
pub fn render_run_record(record: &RunRecord) -> String {
    let outcome = match record.outcome {
        Pi5Outcome::Pass { fixture } => format!("pass fixture={fixture}"),
        Pi5Outcome::ReportedFailure { fixture } => format!("reported-failure fixture={fixture}"),
        Pi5Outcome::Silence => "silence".to_string(),
        Pi5Outcome::SpokeWithoutVerdict { detail } => {
            format!("spoke-without-verdict: {detail}")
        }
    };
    format!(
        "commit\t{}\nfixture\t{}\nport\t{}\nbaud\t{}\nboard_revision\t{}\n\
         firmware_version\t{}\nimage_sha256\t{}\nimage_bytes\t{}\ncapture_sha256\t{}\n\
         capture_bytes\t{}\ncapture_end\t{}\noutcome\t{}\nexit_code\t{}\ntimestamp_unix\t{}\n",
        record.commit,
        record.fixture,
        record.port,
        record.baud,
        record.board_revision,
        record.firmware_version,
        record.image_sha256,
        record.image_bytes,
        record.capture_sha256,
        record.capture_bytes,
        record.capture_end.as_str(),
        outcome,
        outcome_exit(record.outcome) as u8,
        record.timestamp_unix,
    )
}

// ---------------------------------------------------------------------------
// The two seam implementors — the only I/O in this module, deliberately thin
// and deliberately untested here: everything they feed is tested above them.
// ---------------------------------------------------------------------------

/// The real clock.
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    /// A clock whose origin is now.
    pub fn new() -> SystemClock {
        SystemClock { origin: Instant::now() }
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// The real serial source: a reader thread owns the blocking `read` (COM-port
/// reads block for OS-configured intervals the `std` API cannot set), and this
/// end polls the channel, so every timeout decision stays in the tested
/// [`capture`] loop.
pub struct ChannelChunks {
    receiver: mpsc::Receiver<Vec<u8>>,
    poll: Duration,
}

impl ChannelChunks {
    /// Spawns the reader thread over anything `Read` and returns the poll end.
    pub fn spawn<R: Read + Send + 'static>(mut reader: R, poll: Duration) -> ChannelChunks {
        let (sender, receiver) = mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 256];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        if sender.send(buffer[..count].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        ChannelChunks { receiver, poll }
    }
}

impl SerialChunks for ChannelChunks {
    fn next_chunk(&mut self) -> Chunk {
        match self.receiver.recv_timeout(self.poll) {
            Ok(bytes) => Chunk::Bytes(bytes),
            Err(mpsc::RecvTimeoutError::Timeout) => Chunk::Idle,
            Err(mpsc::RecvTimeoutError::Disconnected) => Chunk::Disconnected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    // -- registration (`TEST-P1-07-05-A` clause 5) ---------------------------

    #[test]
    fn the_boot_fixture_is_registered_with_its_owning_test() {
        let fixture = pi5_fixture("boot").expect("the boot fixture must be registered");
        assert_eq!(fixture.owning_test, "TEST-P1-07-01-A");
        assert!(!fixture.summary.is_empty());
    }

    #[test]
    fn every_registered_fixture_names_a_test_document() {
        for fixture in PI5_FIXTURES {
            assert!(
                fixture.owning_test.starts_with("TEST-"),
                "{} names no owning test",
                fixture.name
            );
        }
    }

    #[test]
    fn an_unknown_fixture_resolves_to_nothing() {
        assert_eq!(pi5_fixture("does-not-exist"), None);
        // The Tier 0 namespace is a *different* namespace; accepting its
        // names here would blur exactly the boundary `list-fixtures` prints.
        assert_eq!(pi5_fixture("broken-boot"), None);
    }

    // -- classification (`TEST-P1-07-05-A` clause 3): three outcomes plus pass

    #[test]
    fn zero_bytes_is_silence_not_failure_and_not_success() {
        assert_eq!(classify(b""), Pi5Outcome::Silence);
    }

    #[test]
    fn chatter_without_a_verdict_is_spoke_without_verdict() {
        // The board said something — so the wiring and baud are right — and
        // then stopped before any verdict. During bring-up this is the second
        // most common case after silence and must not read as either.
        let outcome = classify(b"TINYOS-BOOT/1 current_el=EL2 raw=0000000000000008\r\n");
        assert!(matches!(outcome, Pi5Outcome::SpokeWithoutVerdict { .. }), "got: {outcome:?}");
    }

    #[test]
    fn a_well_formed_passing_verdict_is_a_pass_and_names_its_fixture() {
        let text = b"TINYOS-BOOT/1 now_at=EL1\r\nTINYOS-RESULT/1 fixture=boot ok=true\r\n";
        assert_eq!(classify(text), Pi5Outcome::Pass { fixture: "boot".to_string() });
    }

    #[test]
    fn a_well_formed_failing_verdict_is_a_reported_failure() {
        let text = b"TINYOS-RESULT/1 fixture=boot ok=false\r\n";
        assert_eq!(classify(text), Pi5Outcome::ReportedFailure { fixture: "boot".to_string() });
    }

    #[test]
    fn a_corrupt_verdict_line_is_a_failure_to_read_a_verdict_never_a_verdict() {
        // `TEST-P1-07-05-A` clause 4 verbatim. Truthy-looking is not true.
        for text in [
            &b"TINYOS-RESULT/1 fixture=boot ok=yes\r\n"[..],
            &b"TINYOS-RESULT/1 fixture=boot ok=tru"[..],
            &b"TINYOS-RESULT/1 ok=true\r\n"[..],
            &b"TINYOS-RESULT/1 fixture=boot ok=true extra=1\r\n"[..],
        ] {
            let outcome = classify(text);
            assert!(
                matches!(outcome, Pi5Outcome::SpokeWithoutVerdict { .. }),
                "{:?} must fail to read a verdict, got {outcome:?}",
                String::from_utf8_lossy(text)
            );
        }
    }

    #[test]
    fn two_verdict_lines_are_ambiguous_and_therefore_no_verdict() {
        let text =
            b"TINYOS-RESULT/1 fixture=boot ok=true\r\nTINYOS-RESULT/1 fixture=boot ok=false\r\n";
        assert!(matches!(classify(text), Pi5Outcome::SpokeWithoutVerdict { .. }));
    }

    #[test]
    fn non_utf8_noise_around_a_verdict_does_not_defeat_classification() {
        // Hostile input (`BND-03`): a glitched line at the wrong baud puts
        // arbitrary bytes on the wire before the divisors settle. Noise
        // *around* lines must not crash or misread the parser; a verdict on
        // its own clean line still counts.
        let mut text = vec![0xFF, 0xFE, 0x80, b'\r', b'\n'];
        text.extend_from_slice(b"TINYOS-RESULT/1 fixture=boot ok=true\r\n");
        assert_eq!(classify(&text), Pi5Outcome::Pass { fixture: "boot".to_string() });
    }

    #[test]
    fn the_exact_line_hal_arm64_emits_parses_as_a_pass() {
        // Cross-pin with `hal-arm64`'s own `report_result` test: the wire
        // carries CRLF (the PL011 framer owns the CR), and this parser must
        // read exactly those bytes. If either side changes its spelling, one
        // of the two pinned tests goes red.
        let text = b"\r\nTINYOS-BOOT/1 current_el=EL2 raw=0000000000000008\r\nTINYOS-BOOT/1 READY 0123456789ABCDEF\r\nTINYOS-RESULT/1 fixture=boot ok=true\r\n";
        assert_eq!(classify(text), Pi5Outcome::Pass { fixture: "boot".to_string() });
    }

    // -- exit codes (`TEST-P1-07-05-A` clause 2): the Tier 0 scheme, extended

    #[test]
    fn the_exit_scheme_is_the_tier_0_scheme_extended_not_replaced() {
        let pass = outcome_exit(&Pi5Outcome::Pass { fixture: "boot".to_string() });
        let failed = outcome_exit(&Pi5Outcome::ReportedFailure { fixture: "boot".to_string() });
        let silent = outcome_exit(&Pi5Outcome::Silence);
        let spoke =
            outcome_exit(&Pi5Outcome::SpokeWithoutVerdict { detail: "no verdict".to_string() });
        // 0 and 1 keep exactly their `qemu-x86_64` meanings.
        assert_eq!(pass, XtaskExit::KernelBootSucceeded);
        assert_eq!(failed, XtaskExit::KernelBootFailed);
        // The four board outcomes and the harness error are five distinct
        // process exit codes — clause 3: each exits differently.
        let codes =
            [pass as u8, failed as u8, silent as u8, spoke as u8, XtaskExit::HarnessError as u8];
        for (i, a) in codes.iter().enumerate() {
            for b in codes.iter().skip(i + 1) {
                assert_ne!(a, b, "exit codes must be pairwise distinct: {codes:?}");
            }
        }
    }

    // -- the capture loop: bounded, and the three endings are distinct ------

    /// A scripted serial source sharing a scripted clock: each poll advances
    /// the clock by `step_ms` and yields the next scripted event (or `Idle`
    /// forever after).
    struct Script {
        events: VecDeque<Chunk>,
        clock: Rc<Cell<u64>>,
        step_ms: u64,
    }

    impl SerialChunks for Script {
        fn next_chunk(&mut self) -> Chunk {
            self.clock.set(self.clock.get() + self.step_ms);
            self.events.pop_front().unwrap_or(Chunk::Idle)
        }
    }

    struct ScriptClock(Rc<Cell<u64>>);

    impl Clock for ScriptClock {
        fn now_ms(&self) -> u64 {
            self.0.get()
        }
    }

    fn scripted(events: Vec<Chunk>, step_ms: u64) -> (Script, ScriptClock) {
        let shared = Rc::new(Cell::new(0));
        (Script { events: events.into(), clock: Rc::clone(&shared), step_ms }, ScriptClock(shared))
    }

    const POLICY: CapturePolicy =
        CapturePolicy { max_bytes: 4096, overall_ms: 1_000, quiet_ms: 300 };

    #[test]
    fn a_board_that_never_speaks_times_out_with_zero_bytes() {
        let (mut source, clock) = scripted(vec![], 100);
        let (bytes, end) = capture(&mut source, &clock, &POLICY);
        assert!(bytes.is_empty());
        assert_eq!(end, CaptureEnd::OverallTimeout);
        assert_eq!(classify(&bytes), Pi5Outcome::Silence);
    }

    #[test]
    fn the_quiet_window_does_not_end_a_capture_before_the_first_byte() {
        // Silence is the common bring-up case, and the operator may still be
        // walking back from the power switch: only the *overall* deadline may
        // end a byte-less capture, even though quiet_ms (300) elapses three
        // times over first.
        let (mut source, clock) = scripted(vec![], 100);
        let (_, end) = capture(&mut source, &clock, &POLICY);
        assert_eq!(end, CaptureEnd::OverallTimeout);
        assert!(clock.now_ms() >= POLICY.overall_ms, "gave up early at {}ms", clock.now_ms());
    }

    #[test]
    fn a_board_that_speaks_and_stops_ends_on_the_quiet_window() {
        let (mut source, clock) =
            scripted(vec![Chunk::Bytes(b"TINYOS-BOOT/1 current_el=EL2".to_vec())], 100);
        let (bytes, end) = capture(&mut source, &clock, &POLICY);
        assert_eq!(end, CaptureEnd::QuietAfterBytes);
        assert!(!bytes.is_empty());
        assert!(matches!(classify(&bytes), Pi5Outcome::SpokeWithoutVerdict { .. }));
    }

    #[test]
    fn a_complete_verdict_line_ends_the_capture_without_waiting_out_the_quiet_window() {
        let (mut source, clock) =
            scripted(vec![Chunk::Bytes(b"TINYOS-RESULT/1 fixture=boot ok=true\r\n".to_vec())], 100);
        let (bytes, end) = capture(&mut source, &clock, &POLICY);
        assert_eq!(end, CaptureEnd::VerdictSeen);
        assert_eq!(classify(&bytes), Pi5Outcome::Pass { fixture: "boot".to_string() });
        assert!(clock.now_ms() < POLICY.quiet_ms, "waited {}ms for nothing", clock.now_ms());
    }

    #[test]
    fn a_verdict_split_across_chunks_is_assembled_before_being_judged() {
        let (mut source, clock) = scripted(
            vec![
                Chunk::Bytes(b"TINYOS-RESULT/1 fixture=".to_vec()),
                Chunk::Bytes(b"boot ok=true\r\n".to_vec()),
            ],
            10,
        );
        let (bytes, end) = capture(&mut source, &clock, &POLICY);
        assert_eq!(end, CaptureEnd::VerdictSeen);
        assert_eq!(classify(&bytes), Pi5Outcome::Pass { fixture: "boot".to_string() });
    }

    #[test]
    fn a_verdict_without_its_newline_is_not_yet_a_verdict() {
        // The line could still grow a corrupting continuation; judging it
        // early would be guessing. The quiet window is what ends this one.
        let (mut source, clock) =
            scripted(vec![Chunk::Bytes(b"TINYOS-RESULT/1 fixture=boot ok=true".to_vec())], 100);
        let (_, end) = capture(&mut source, &clock, &POLICY);
        assert_eq!(end, CaptureEnd::QuietAfterBytes);
    }

    #[test]
    fn a_board_stuck_in_an_output_loop_cannot_exhaust_the_host() {
        // `SEC-20`. 100 chunks of 1024 bytes against a 4096-byte cap.
        let events: Vec<Chunk> = (0..100).map(|_| Chunk::Bytes(vec![b'A'; 1024])).collect();
        let (mut source, clock) = scripted(events, 1);
        let (bytes, end) = capture(&mut source, &clock, &POLICY);
        assert_eq!(end, CaptureEnd::ByteCapReached);
        assert_eq!(bytes.len(), POLICY.max_bytes);
    }

    #[test]
    fn a_source_that_dies_reports_disconnected_not_a_verdict() {
        let (mut source, clock) =
            scripted(vec![Chunk::Bytes(b"TINYOS-BOOT/1 ".to_vec()), Chunk::Disconnected], 10);
        let (bytes, end) = capture(&mut source, &clock, &POLICY);
        assert_eq!(end, CaptureEnd::Disconnected);
        assert!(!bytes.is_empty());
    }

    #[test]
    fn verdict_detection_needs_the_sentinel_and_a_terminating_newline() {
        assert!(!contains_complete_verdict_line(b""));
        assert!(!contains_complete_verdict_line(b"TINYOS-BOOT/1 READY\r\n"));
        assert!(!contains_complete_verdict_line(b"TINYOS-RESULT/1 fixture=boot ok=true"));
        assert!(contains_complete_verdict_line(b"TINYOS-RESULT/1 fixture=boot ok=true\n"));
        assert!(contains_complete_verdict_line(
            b"noise\r\nTINYOS-RESULT/1 fixture=boot ok=false\r\nmore"
        ));
    }

    // -- the image build is not folklore (`TEST-P1-07-05-A` clause 1) --------

    /// Builds a minimal but well-formed little-endian ELF64 for AArch64 with
    /// the given entry point and `(paddr, bytes)` PT_LOAD segments.
    fn synthetic_elf(entry: u64, segments: &[(u64, &[u8])]) -> Vec<u8> {
        const EHSIZE: usize = 64;
        const PHENTSIZE: usize = 56;
        let phoff = EHSIZE;
        let data_start = phoff + segments.len() * PHENTSIZE;

        let mut elf = vec![0u8; data_start];
        elf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
        elf[4] = 2; // ELFCLASS64
        elf[5] = 1; // little-endian
        elf[6] = 1; // EV_CURRENT
        elf[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
        elf[18..20].copy_from_slice(&183u16.to_le_bytes()); // EM_AARCH64
        elf[20..24].copy_from_slice(&1u32.to_le_bytes()); // e_version
        elf[24..32].copy_from_slice(&entry.to_le_bytes()); // e_entry
        elf[32..40].copy_from_slice(&(phoff as u64).to_le_bytes()); // e_phoff
        elf[52..54].copy_from_slice(&(EHSIZE as u16).to_le_bytes()); // e_ehsize
        elf[54..56].copy_from_slice(&(PHENTSIZE as u16).to_le_bytes()); // e_phentsize
        elf[56..58].copy_from_slice(&(segments.len() as u16).to_le_bytes()); // e_phnum

        let mut offset = data_start;
        for (index, (paddr, bytes)) in segments.iter().enumerate() {
            let base = phoff + index * PHENTSIZE;
            elf[base..base + 4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
            elf[base + 8..base + 16].copy_from_slice(&(offset as u64).to_le_bytes()); // p_offset
            elf[base + 16..base + 24].copy_from_slice(&paddr.to_le_bytes()); // p_vaddr
            elf[base + 24..base + 32].copy_from_slice(&paddr.to_le_bytes()); // p_paddr
            elf[base + 32..base + 40].copy_from_slice(&(bytes.len() as u64).to_le_bytes()); // p_filesz
            elf[base + 40..base + 48].copy_from_slice(&(bytes.len() as u64).to_le_bytes()); // p_memsz
            offset += bytes.len();
        }
        for (_, bytes) in segments {
            elf.extend_from_slice(bytes);
        }
        elf
    }

    #[test]
    fn a_well_formed_elf_flattens_with_the_entry_point_as_the_first_byte() {
        let elf = synthetic_elf(
            KERNEL_LOAD_ADDRESS,
            &[(KERNEL_LOAD_ADDRESS, &[0xA4, 0x00, 0x38, 0xD5][..])],
        );
        let image = flatten_elf(&elf).expect("well-formed");
        // The firmware jumps to the first byte of the file; the divergence
        // record's layout check pinned these exact bytes (`mrs x4, MPIDR_EL1`).
        assert_eq!(image.bytes, vec![0xA4, 0x00, 0x38, 0xD5]);
        assert_eq!(image.entry, KERNEL_LOAD_ADDRESS);
        assert_eq!(image.load_address, KERNEL_LOAD_ADDRESS);
    }

    #[test]
    fn a_gap_between_segments_is_zero_filled() {
        let elf = synthetic_elf(
            KERNEL_LOAD_ADDRESS,
            &[(KERNEL_LOAD_ADDRESS, &[0x11, 0x22][..]), (KERNEL_LOAD_ADDRESS + 6, &[0x33][..])],
        );
        let image = flatten_elf(&elf).expect("well-formed");
        assert_eq!(image.bytes, vec![0x11, 0x22, 0, 0, 0, 0, 0x33]);
    }

    #[test]
    fn an_entry_point_that_is_not_the_load_address_is_rejected() {
        // `ENTRY()` does not survive flattening: if the entry is not byte
        // zero, the firmware starts execution in the middle of the image.
        let elf =
            synthetic_elf(KERNEL_LOAD_ADDRESS + 0x50, &[(KERNEL_LOAD_ADDRESS, &[0u8; 8][..])]);
        let error = flatten_elf(&elf).expect_err("must reject");
        assert!(error.contains("entry"), "unhelpful error: {error}");
    }

    #[test]
    fn a_load_address_other_than_0x80000_is_rejected() {
        // Divergence record §3: the firmware loads at 0x80000 only with
        // `os_check=0`; an image linked anywhere else executes garbage.
        let elf = synthetic_elf(0x20_0000, &[(0x20_0000, &[0u8; 8][..])]);
        let error = flatten_elf(&elf).expect_err("must reject");
        assert!(error.contains("80000") || error.contains("load"), "unhelpful error: {error}");
    }

    #[test]
    fn a_truncated_elf_fails_with_a_reason_rather_than_panicking() {
        let whole = synthetic_elf(KERNEL_LOAD_ADDRESS, &[(KERNEL_LOAD_ADDRESS, &[0xAB; 32][..])]);
        for cut in [0, 3, 16, 63, 64, whole.len() - 1] {
            assert!(flatten_elf(&whole[..cut]).is_err(), "accepted a {cut}-byte prefix");
        }
    }

    #[test]
    fn bytes_that_are_not_an_elf_are_rejected() {
        assert!(flatten_elf(b"MZ this is a PE, not an ELF").is_err());
        assert!(flatten_elf(&[0x7F, b'E', b'L', b'F', 1 /* 32-bit */]).is_err());
    }

    #[test]
    fn an_image_larger_than_the_ceiling_is_rejected_not_materialised() {
        // A segment placed 32 MiB away would zero-fill a file that size; the
        // ceiling turns a linker-script mistake into an error message.
        let elf = synthetic_elf(
            KERNEL_LOAD_ADDRESS,
            &[
                (KERNEL_LOAD_ADDRESS, &[0x11][..]),
                (KERNEL_LOAD_ADDRESS + 32 * 1024 * 1024, &[0x22][..]),
            ],
        );
        assert!(flatten_elf(&elf).is_err());
    }

    // -- attribution (`TEST-P1-07-05-A` clause 7) ----------------------------

    #[test]
    fn sha256_matches_the_fips_test_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // One multi-block input, so the padding path over 64 bytes is covered.
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn the_run_record_carries_every_fact_a_report_needs_to_trace_the_capture() {
        let outcome = Pi5Outcome::Pass { fixture: "boot".to_string() };
        let record = RunRecord {
            commit: "b423fa9",
            fixture: "boot",
            port: "COM7",
            baud: 115_200,
            board_revision: "d04170",
            firmware_version: "unrecorded",
            image_sha256: "aa",
            image_bytes: 62_000,
            capture_sha256: "bb",
            capture_bytes: 512,
            capture_end: CaptureEnd::VerdictSeen,
            outcome: &outcome,
            timestamp_unix: 1_785_000_000,
        };
        let rendered = render_run_record(&record);
        for needle in [
            "commit\tb423fa9",
            "fixture\tboot",
            "port\tCOM7",
            "baud\t115200",
            "board_revision\td04170",
            "firmware_version\tunrecorded",
            "image_sha256\taa",
            "image_bytes\t62000",
            "capture_sha256\tbb",
            "capture_bytes\t512",
            "capture_end\tverdict-seen",
            "exit_code\t0",
            "timestamp_unix\t1785000000",
        ] {
            assert!(rendered.contains(needle), "missing `{needle}` in:\n{rendered}");
        }
        // Machine-readable: every line is exactly `key<TAB>value`.
        for line in rendered.lines() {
            assert_eq!(line.split('\t').count(), 2, "not a key-value pair: {line:?}");
        }
    }

    // -- placement (`TEST-P1-07-05-A` clause 1): nothing is folklore ---------

    #[test]
    fn the_placement_instructions_name_every_fact_the_divergence_record_pinned() {
        let text = placement_instructions(62_000, "abc123");
        for needle in
            ["kernel8.img", "os_check=0", "kernel=kernel8.img", "config.txt", "115200", "abc123"]
        {
            assert!(text.contains(needle), "missing `{needle}` in:\n{text}");
        }
    }
}
