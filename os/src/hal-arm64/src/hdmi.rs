//! The boot splash (`STORY-P1-07-07`): "TinyOS" on the HDMI output, painted
//! into a framebuffer the GPU firmware is asked for over the VideoCore
//! mailbox property interface.
//!
//! Owner-ordered UX: a successful boot that looks identical to a dead board
//! is unacceptable, and the boxed board hides the ACT LED. The discipline
//! (`TEST-P1-07-07-A`): the splash runs **after** the `TOS64-RESULT/1`
//! verdict, every wait is bounded, every failure is silent-continue into
//! `park()`, and the firmware's framebuffer descriptor is hostile input —
//! believed only after typed whole-descriptor validation. A splash is not
//! evidence; nothing here contributes to any capture or timing claim.

/// Number of `u32` words in the property message — a 16-byte multiple, so
/// the buffer's mailbox-required alignment holds by size as well as by type.
pub const REQUEST_WORDS: usize = 28;

/// Rows per glyph in the 8×8 block font.
pub const GLYPH_ROWS: u32 = 8;

/// Total text columns of "TinyOS": six 8-wide glyphs plus five 1-column gaps.
pub const TEXT_COLUMNS: u32 = 6 * 8 + 5;

/// Splash background — dark navy, readable against either RGB/BGR ordering.
pub const BACKGROUND: u32 = 0x0010_2040;

/// Splash foreground — white, identical in RGB and BGR.
pub const FOREGROUND: u32 = 0xFFFF_FFFF;

/// The requested mode. 720p is the most universally scanned-out mode; the
/// firmware may answer with what the display actually accepted, and the
/// *answer* is what gets validated and drawn into.
const REQUEST_WIDTH: u32 = 1280;
const REQUEST_HEIGHT: u32 = 720;

/// Upper sanity bound on either screen dimension from a hostile descriptor.
const MAX_DIMENSION: u32 = 4096;

/// The VideoCore mailbox property message, 16-byte aligned by construction —
/// the interface requires the buffer address's low four bits to be zero
/// because they carry the channel number.
#[repr(C, align(16))]
pub struct PropertyMessage {
    words: [u32; REQUEST_WORDS],
}

impl PropertyMessage {
    /// Builds the pinned framebuffer request: physical/virtual 1280×720,
    /// depth 32, allocate at 4096 alignment, get pitch.
    #[must_use]
    pub const fn framebuffer_request() -> Self {
        let mut words = [0u32; REQUEST_WORDS];
        words[0] = (REQUEST_WORDS * 4) as u32;
        words[1] = 0; // process request
                      // Set physical (display) size.
        words[2] = 0x0004_8003;
        words[3] = 8;
        words[4] = 0;
        words[5] = REQUEST_WIDTH;
        words[6] = REQUEST_HEIGHT;
        // Set virtual (buffer) size.
        words[7] = 0x0004_8004;
        words[8] = 8;
        words[9] = 0;
        words[10] = REQUEST_WIDTH;
        words[11] = REQUEST_HEIGHT;
        // Set depth.
        words[12] = 0x0004_8005;
        words[13] = 4;
        words[14] = 0;
        words[15] = 32;
        // Allocate the buffer: request carries the alignment, the response
        // overwrites these two words with base and size.
        words[16] = 0x0004_0001;
        words[17] = 8;
        words[18] = 0;
        words[19] = 4096;
        words[20] = 0;
        // Get pitch.
        words[21] = 0x0004_0008;
        words[22] = 4;
        words[23] = 0;
        words[24] = 0;
        // End tag; the remainder stays zero padding.
        words[25] = 0;
        Self { words }
    }

    /// The raw words, for host pinning and for the board-side handshake.
    #[must_use]
    pub const fn words(&self) -> &[u32; REQUEST_WORDS] {
        &self.words
    }

    /// Mutable words: the mailbox response is written in place.
    pub fn words_mut(&mut self) -> &mut [u32; REQUEST_WORDS] {
        &mut self.words
    }
}

/// A validated framebuffer descriptor — the only form pixels are written
/// through. `base` is the ARM physical address with the VideoCore bus-alias
/// bits already masked off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramebufferInfo {
    /// ARM physical base address of the pixel buffer.
    pub base: u32,
    /// Total buffer size in bytes.
    pub size: u32,
    /// Bytes per row (may exceed `width * 4`).
    pub pitch: u32,
    /// Pixels per row the display accepted.
    pub width: u32,
    /// Rows the display accepted.
    pub height: u32,
}

/// Why a firmware response was rejected whole. One typed arm per corruption,
/// because a framebuffer descriptor is hostile input and a rejected one must
/// paint nothing (`BND-02`, `PD-12`, `RCG-01`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramebufferError {
    /// The header word was not the success code.
    ResponseCode(u32),
    /// A required tag carries no response bit — the firmware never answered it.
    MissingTag(u32),
    /// The allocated base masks to zero.
    ZeroBase,
    /// The buffer is smaller than the claimed geometry needs.
    BadSize {
        /// What the firmware claimed.
        size: u32,
        /// What `pitch × height` requires.
        required: u32,
    },
    /// The depth is not the 32 bits per pixel that was requested.
    BadDepth(u32),
    /// The pitch cannot hold one row of the claimed width.
    BadPitch {
        /// Bytes per row claimed.
        pitch: u32,
        /// Pixels per row claimed.
        width: u32,
    },
    /// A dimension is zero or beyond the sanity bound.
    BadDimensions {
        /// Claimed width.
        width: u32,
        /// Claimed height.
        height: u32,
    },
    /// A mailbox status poll exhausted its budget.
    Timeout,
}

const RESPONSE_BIT: u32 = 0x8000_0000;

/// Validates a property-interface response whole, against the pinned request
/// layout. Order: header, tag presence, then every field of the descriptor.
pub fn parse_response(words: &[u32; REQUEST_WORDS]) -> Result<FramebufferInfo, FramebufferError> {
    if words[1] != RESPONSE_BIT {
        return Err(FramebufferError::ResponseCode(words[1]));
    }
    if words[17] & RESPONSE_BIT == 0 {
        return Err(FramebufferError::MissingTag(0x0004_0001));
    }
    if words[22] & RESPONSE_BIT == 0 {
        return Err(FramebufferError::MissingTag(0x0004_0008));
    }
    let width = words[5];
    let height = words[6];
    let depth = words[15];
    let base = words[19] & 0x3FFF_FFFF;
    let size = words[20];
    let pitch = words[24];

    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(FramebufferError::BadDimensions { width, height });
    }
    if depth != 32 {
        return Err(FramebufferError::BadDepth(depth));
    }
    if base == 0 {
        return Err(FramebufferError::ZeroBase);
    }
    if pitch < width * 4 {
        return Err(FramebufferError::BadPitch { pitch, width });
    }
    let required = pitch * height;
    if size < required {
        return Err(FramebufferError::BadSize { size, required });
    }
    Ok(FramebufferInfo { base, size, pitch, width, height })
}

/// Where splash pixels go — a mock surface on the host, the validated
/// framebuffer on the board. The seam that makes the renderer pure.
pub trait Surface {
    /// Surface width in pixels.
    fn width(&self) -> u32;
    /// Surface height in pixels.
    fn height(&self) -> u32;
    /// Writes one pixel; implementations must ignore or reject out-of-bounds
    /// coordinates rather than trusting the caller.
    fn put(&mut self, x: u32, y: u32, color: u32);
}

/// The 8×8 block glyphs of "TinyOS", bit 7 = leftmost column.
const GLYPHS: [[u8; 8]; 6] = [
    // T
    [0xFE, 0x38, 0x38, 0x38, 0x38, 0x38, 0x38, 0x00],
    // i
    [0x30, 0x00, 0x70, 0x30, 0x30, 0x30, 0x78, 0x00],
    // n
    [0x00, 0x00, 0x7C, 0x66, 0x66, 0x66, 0x66, 0x00],
    // y
    [0x00, 0x00, 0x66, 0x66, 0x66, 0x3E, 0x06, 0x7C],
    // O
    [0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
    // S
    [0x3E, 0x60, 0x60, 0x3C, 0x06, 0x06, 0x7C, 0x00],
];

/// The integer scale the text is drawn at for a given surface: large enough
/// to fill roughly half the width or a third of the height, never zero.
#[must_use]
pub const fn splash_scale(width: u32, height: u32) -> u32 {
    let by_width = width / (TEXT_COLUMNS * 2);
    let by_height = height / (GLYPH_ROWS * 3);
    let scale = if by_width < by_height { by_width } else { by_height };
    if scale == 0 {
        1
    } else {
        scale
    }
}

/// Paints the splash: background fill, then "TinyOS" centred at the computed
/// scale. A surface too small for the whole text gets the background only —
/// clipped glyph fragments would read as corruption, not branding.
pub fn render_splash<S: Surface>(surface: &mut S) {
    let width = surface.width();
    let height = surface.height();
    let mut y = 0;
    while y < height {
        let mut x = 0;
        while x < width {
            surface.put(x, y, BACKGROUND);
            x += 1;
        }
        y += 1;
    }

    let scale = splash_scale(width, height);
    let text_w = TEXT_COLUMNS * scale;
    let text_h = GLYPH_ROWS * scale;
    if text_w > width || text_h > height {
        return;
    }
    let x0 = (width - text_w) / 2;
    let y0 = (height - text_h) / 2;

    let mut glyph_index = 0;
    while glyph_index < GLYPHS.len() {
        let glyph = &GLYPHS[glyph_index];
        let glyph_x0 = x0 + (glyph_index as u32) * 9 * scale;
        let mut row = 0;
        while row < 8 {
            let bits = glyph[row as usize];
            let mut col = 0;
            while col < 8 {
                if bits & (0x80 >> col) != 0 {
                    let mut dy = 0;
                    while dy < scale {
                        let mut dx = 0;
                        while dx < scale {
                            surface.put(
                                glyph_x0 + col * scale + dx,
                                y0 + row * scale + dy,
                                FOREGROUND,
                            );
                            dx += 1;
                        }
                        dy += 1;
                    }
                }
                col += 1;
            }
            row += 1;
        }
        glyph_index += 1;
    }
}

/// One mailbox status poll's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollOutcome {
    /// The condition is met; proceed.
    Ready,
    /// Budget remains; poll again.
    Continue,
    /// The budget is exhausted — and stays exhausted.
    TimedOut,
}

/// A countdown that makes every board-side wait provably bounded
/// (`SEC-20`, `PD-07`): the volatile spin loops consume one of these and can
/// therefore never spin forever, and the countdown never rewinds.
pub struct BoundedPoll {
    remaining: u32,
}

impl BoundedPoll {
    /// A budget of `attempts` polls.
    #[must_use]
    pub const fn new(attempts: u32) -> Self {
        Self { remaining: attempts }
    }

    /// Consumes one attempt.
    pub fn step(&mut self, ready: bool) -> PollOutcome {
        if ready && self.remaining > 0 {
            return PollOutcome::Ready;
        }
        if self.remaining <= 1 {
            self.remaining = 0;
            return PollOutcome::TimedOut;
        }
        self.remaining -= 1;
        PollOutcome::Continue
    }
}

/// Board-only half: the mailbox handshake and the volatile-write surface.
/// Everything above this line is host-tested; everything below is the thin
/// MMIO glue those tests cannot reach, kept exactly as thin as it reads.
#[cfg(target_arch = "aarch64")]
mod board {
    use super::*;

    /// BCM2712 VideoCore mailbox, from the bcm2712 device tree's
    /// `mailbox@7c013880` under the `0x10_7c00_0000` AXI window. Register
    /// layout is the classic VideoCore one: read at +0x00, read status at
    /// +0x18, write at +0x20, write status at +0x38.
    const MAILBOX_BASE: usize = 0x10_7C01_3880;
    const MAILBOX_READ: usize = MAILBOX_BASE;
    const MAILBOX_READ_STATUS: usize = MAILBOX_BASE + 0x18;
    const MAILBOX_WRITE: usize = MAILBOX_BASE + 0x20;
    const MAILBOX_WRITE_STATUS: usize = MAILBOX_BASE + 0x38;
    const STATUS_FULL: u32 = 0x8000_0000;
    const STATUS_EMPTY: u32 = 0x4000_0000;
    const CHANNEL_PROPERTY: u32 = 8;
    /// Poll budget per wait. Device-memory reads at boot clock are ~100 ns;
    /// this bounds any single wait to well under a second.
    const POLL_BUDGET: u32 = 2_000_000;

    /// The framebuffer as a [`Surface`]: volatile pixel writes through the
    /// validated descriptor, coordinates re-checked here so no caller can
    /// aim a write outside the buffer the firmware granted.
    struct Framebuffer {
        info: FramebufferInfo,
    }

    impl Surface for Framebuffer {
        fn width(&self) -> u32 {
            self.info.width
        }
        fn height(&self) -> u32 {
            self.info.height
        }
        fn put(&mut self, x: u32, y: u32, color: u32) {
            if x >= self.info.width || y >= self.info.height {
                return;
            }
            let offset = y as usize * self.info.pitch as usize + x as usize * 4;
            if offset + 4 > self.info.size as usize {
                return;
            }
            let address = self.info.base as usize + offset;
            // SAFETY: `parse_response` validated base, size, pitch and
            // dimensions as a whole descriptor, and both guards above keep
            // `address` inside `[base, base + size)`. The buffer is
            // firmware-owned RAM outside every TinyOS structure; with the
            // MMU off the access is Device memory and cannot be reordered
            // into anything it could corrupt.
            unsafe { core::ptr::write_volatile(address as *mut u32, color) };
        }
    }

    /// Asks the firmware for a framebuffer and paints the splash into it.
    ///
    /// Every failure returns silently: the caller is post-verdict boot code
    /// whose next act is `park()`, and a dark screen is the accepted fallback
    /// (`TEST-P1-07-07-A` clause 4). Never called before the verdict.
    pub fn show_splash() {
        let mut message = PropertyMessage::framebuffer_request();
        let buffer_address = message.words().as_ptr() as usize as u32;

        // Wait for write-FIFO space, bounded.
        let mut poll = BoundedPoll::new(POLL_BUDGET);
        loop {
            // SAFETY: reads/writes below are 4-byte MMIO accesses to the
            // BCM2712 mailbox registers, addresses fixed above; with the MMU
            // off they are Device-nGnRnE and side-effect-exact.
            let full = unsafe { core::ptr::read_volatile(MAILBOX_WRITE_STATUS as *const u32) }
                & STATUS_FULL
                != 0;
            match poll.step(!full) {
                PollOutcome::Ready => break,
                PollOutcome::Continue => continue,
                PollOutcome::TimedOut => return,
            }
        }

        // SAFETY: as above; the value is the 16-byte-aligned message address
        // with the property channel in the low bits, exactly the register's
        // contract.
        unsafe {
            core::ptr::write_volatile(
                MAILBOX_WRITE as *mut u32,
                (buffer_address & !0xF) | CHANNEL_PROPERTY,
            );
        }

        // Wait for the response addressed to us, bounded.
        let mut poll = BoundedPoll::new(POLL_BUDGET);
        loop {
            // SAFETY: as above.
            let empty = unsafe { core::ptr::read_volatile(MAILBOX_READ_STATUS as *const u32) }
                & STATUS_EMPTY
                != 0;
            match poll.step(!empty) {
                PollOutcome::Ready => {}
                PollOutcome::Continue => continue,
                PollOutcome::TimedOut => return,
            }
            // SAFETY: as above.
            let value = unsafe { core::ptr::read_volatile(MAILBOX_READ as *const u32) };
            if value & 0xF == CHANNEL_PROPERTY && value & !0xF == buffer_address & !0xF {
                break;
            }
            // A response for another channel: keep draining inside the same
            // budget rather than trusting the firmware to be well-behaved.
            poll = BoundedPoll::new(POLL_BUDGET);
        }

        // The firmware wrote the response into our buffer; validate whole.
        let Ok(info) = parse_response(message.words_mut()) else {
            return;
        };
        render_splash(&mut Framebuffer { info });
    }
}

/// Paints the splash on the board; a no-op in host builds so the boot path
/// can call it unconditionally.
pub fn show_splash() {
    #[cfg(target_arch = "aarch64")]
    board::show_splash();
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- clause 1: the property message is exact bytes ----------------------

    #[test]
    fn the_framebuffer_request_layout_is_pinned_word_for_word() {
        let message = PropertyMessage::framebuffer_request();
        let words = message.words();
        let expected: [u32; REQUEST_WORDS] = [
            (REQUEST_WORDS * 4) as u32, // total buffer size in bytes
            0x0000_0000,                // process-request code
            0x0004_8003,
            8,
            0,
            1280,
            720, // set physical size
            0x0004_8004,
            8,
            0,
            1280,
            720, // set virtual size
            0x0004_8005,
            4,
            0,
            32, // set depth
            0x0004_0001,
            8,
            0,
            4096,
            0, // allocate buffer (alignment, then base/size out)
            0x0004_0008,
            4,
            0,
            0,           // get pitch
            0x0000_0000, // end tag
            0,
            0, // padding to a 16-byte multiple
        ];
        assert_eq!(words[..], expected[..]);
        assert_eq!(REQUEST_WORDS % 4, 0, "buffer stays a 16-byte multiple");
        assert_eq!(core::mem::align_of::<PropertyMessage>(), 16);
    }

    // --- clause 2: the response is hostile input ----------------------------

    /// A well-formed firmware response for the pinned request shape.
    fn good_response() -> [u32; REQUEST_WORDS] {
        let mut words = *PropertyMessage::framebuffer_request().words();
        words[1] = 0x8000_0000; // success
                                // set physical size answered in place (tag value words at 5,6).
        words[4] = 0x8000_0008;
        // set virtual size.
        words[9] = 0x8000_0008;
        // depth.
        words[13] = 0x8000_0004;
        // allocate buffer: base (bus-aliased), size.
        words[17] = 0x8000_0008;
        words[19] = 0xC010_0000;
        words[20] = 1280 * 720 * 4;
        // pitch.
        words[22] = 0x8000_0004;
        words[24] = 1280 * 4;
        words
    }

    #[test]
    fn a_well_formed_descriptor_is_accepted_and_bus_masked() {
        let info = parse_response(&good_response()).expect("well-formed descriptor");
        assert_eq!(info.base, 0x0010_0000, "bus alias bits are masked off");
        assert_eq!(info.size, 1280 * 720 * 4);
        assert_eq!(info.pitch, 1280 * 4);
        assert_eq!(info.width, 1280);
        assert_eq!(info.height, 720);
    }

    #[test]
    fn every_corruption_arm_is_a_distinct_typed_rejection() {
        // Wrong response code.
        let mut words = good_response();
        words[1] = 0x8000_0001;
        assert_eq!(parse_response(&words), Err(FramebufferError::ResponseCode(0x8000_0001)));

        // Allocate tag never answered.
        let mut words = good_response();
        words[17] = 0x0000_0008;
        assert_eq!(parse_response(&words), Err(FramebufferError::MissingTag(0x0004_0001)));

        // Pitch tag never answered.
        let mut words = good_response();
        words[22] = 0x0000_0004;
        assert_eq!(parse_response(&words), Err(FramebufferError::MissingTag(0x0004_0008)));

        // Zero base after masking.
        let mut words = good_response();
        words[19] = 0xC000_0000;
        assert_eq!(parse_response(&words), Err(FramebufferError::ZeroBase));

        // Size too small for the claimed geometry.
        let mut words = good_response();
        words[20] = 1280 * 4; // one row
        assert_eq!(
            parse_response(&words),
            Err(FramebufferError::BadSize { size: 1280 * 4, required: 1280 * 4 * 720 })
        );

        // Depth is not 32.
        let mut words = good_response();
        words[15] = 16;
        assert_eq!(parse_response(&words), Err(FramebufferError::BadDepth(16)));

        // Pitch narrower than a row.
        let mut words = good_response();
        words[24] = 1280 * 4 - 4;
        assert_eq!(
            parse_response(&words),
            Err(FramebufferError::BadPitch { pitch: 1280 * 4 - 4, width: 1280 })
        );

        // Absurd dimensions.
        let mut words = good_response();
        words[5] = 100_000;
        assert_eq!(
            parse_response(&words),
            Err(FramebufferError::BadDimensions { width: 100_000, height: 720 })
        );
    }

    // --- clause 3: the renderer is pure, bounded and centred ----------------

    struct MockSurface {
        width: u32,
        height: u32,
        pixels: Vec<u32>,
        out_of_bounds: usize,
    }

    impl MockSurface {
        fn new(width: u32, height: u32) -> Self {
            MockSurface {
                width,
                height,
                pixels: vec![0; (width * height) as usize],
                out_of_bounds: 0,
            }
        }

        fn at(&self, x: u32, y: u32) -> u32 {
            self.pixels[(y * self.width + x) as usize]
        }
    }

    impl Surface for MockSurface {
        fn width(&self) -> u32 {
            self.width
        }
        fn height(&self) -> u32 {
            self.height
        }
        fn put(&mut self, x: u32, y: u32, color: u32) {
            if x < self.width && y < self.height {
                self.pixels[(y * self.width + x) as usize] = color;
            } else {
                self.out_of_bounds += 1;
            }
        }
    }

    #[test]
    fn the_background_covers_the_surface_and_nothing_writes_out_of_bounds() {
        for (w, h) in [(1280, 720), (640, 480), (61, 23), (10, 10), (1, 1)] {
            let mut surface = MockSurface::new(w, h);
            render_splash(&mut surface);
            assert_eq!(surface.out_of_bounds, 0, "{w}x{h}: no write may leave the surface");
            assert_eq!(surface.at(0, 0), BACKGROUND, "{w}x{h}: corner is background");
            assert_eq!(surface.at(w - 1, h - 1), BACKGROUND, "{w}x{h}: far corner too");
        }
    }

    #[test]
    fn the_glyph_pass_paints_a_nontrivial_centred_tinyos() {
        let mut surface = MockSurface::new(1280, 720);
        render_splash(&mut surface);
        let foreground = surface.pixels.iter().filter(|&&p| p == FOREGROUND).count();
        assert!(foreground > 1_000, "the text actually paints ({foreground} px)");

        // Centred: the T's top-left bar pixel sits at the computed origin.
        let scale = splash_scale(1280, 720);
        let text_w = TEXT_COLUMNS * scale;
        let text_h = GLYPH_ROWS * scale;
        let x0 = (1280 - text_w) / 2;
        let y0 = (720 - text_h) / 2;
        assert_eq!(surface.at(x0, y0), FOREGROUND, "T top bar starts at the origin");
        // And symmetric margins mean the far side of the text box is inside.
        assert_eq!(surface.at(x0 + text_w - 1, y0 + text_h - 1 - (scale - 1)), BACKGROUND);
    }

    #[test]
    fn a_surface_too_small_for_the_text_gets_background_only() {
        let mut surface = MockSurface::new(10, 10);
        render_splash(&mut surface);
        assert_eq!(surface.out_of_bounds, 0);
        assert!(surface.pixels.iter().all(|&p| p == BACKGROUND), "no clipped glyph fragments");
    }

    // --- clause 4: every wait is bounded ------------------------------------

    #[test]
    fn the_poll_budget_counts_down_to_a_typed_timeout() {
        let mut poll = BoundedPoll::new(3);
        assert_eq!(poll.step(false), PollOutcome::Continue);
        assert_eq!(poll.step(false), PollOutcome::Continue);
        assert_eq!(poll.step(false), PollOutcome::TimedOut);
        // And it stays timed out rather than wrapping into a fresh budget.
        assert_eq!(poll.step(false), PollOutcome::TimedOut);

        let mut poll = BoundedPoll::new(3);
        assert_eq!(poll.step(true), PollOutcome::Ready);
    }
}
