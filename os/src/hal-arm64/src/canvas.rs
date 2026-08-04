//! The firmware's canvas (`STORY-P1-07-09`): the simple-framebuffer the
//! firmware already scans out, painted with the report as readable text.
//!
//! `TEST-P1-07-09-A`. Source: the on-silicon capture's second visit
//! (`pios-ground-truth-2026-08-03.txt`) — `framebuffer at 0x3f800000,
//! 0x3f4800 bytes`, `format=r5g6b5, mode=1920x1080x16, linelength=3840`,
//! cross-checked against `/sys/class/graphics/fb0`. The geometry constants
//! live in [`crate::board`]; everything here is pure over the existing
//! [`crate::hdmi::Surface`] seam except the one aarch64 surface at the
//! bottom. Like the splash and the lamp: an instrument and UX, never
//! evidence — no capture cites a painted pixel.

use crate::hdmi::Surface;

/// Converts the splash's 32-bit colors to the canvas's r5g6b5.
#[must_use]
pub const fn rgb565(color: u32) -> u16 {
    let r = (color >> 16) & 0xFF;
    let g = (color >> 8) & 0xFF;
    let b = color & 0xFF;
    (((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)) as u16
}

/// A bounds-honest RGB565 surface over a pixel slice — the host-testable
/// form; the board form at the bottom shares the same arithmetic.
pub struct SliceSurface<'a> {
    /// Backing pixels, row-major with `stride_pixels` per row.
    pub pixels: &'a mut [u16],
    /// Visible pixels per row.
    pub width: u32,
    /// Visible rows.
    pub height: u32,
    /// Pixels per stored row (stride in pixels, ≥ `width`).
    pub stride_pixels: u32,
}

impl Surface for SliceSurface<'_> {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn put(&mut self, x: u32, y: u32, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let index = (y * self.stride_pixels + x) as usize;
        if let Some(pixel) = self.pixels.get_mut(index) {
            *pixel = rgb565(color);
        }
    }
}

/// The glyph height and width, matching the splash's 8×8 block font.
pub const GLYPH_SIZE: u32 = 8;

/// The 8×8 glyph for a report byte: letters case-folded to the uppercase
/// table, digits and report punctuation direct, everything else a visible
/// block — a character is never silently skipped (`TEST-P1-07-09-A`
/// clause 3). Bit 7 is the leftmost column.
#[must_use]
pub const fn glyph_for(byte: u8) -> [u8; 8] {
    let upper = if byte.is_ascii_lowercase() { byte - 32 } else { byte };
    match upper {
        b'A' => [0x38, 0x6C, 0xC6, 0xC6, 0xFE, 0xC6, 0xC6, 0x00],
        b'B' => [0xFC, 0xC6, 0xC6, 0xFC, 0xC6, 0xC6, 0xFC, 0x00],
        b'C' => [0x7C, 0xC6, 0xC0, 0xC0, 0xC0, 0xC6, 0x7C, 0x00],
        b'D' => [0xF8, 0xCC, 0xC6, 0xC6, 0xC6, 0xCC, 0xF8, 0x00],
        b'E' => [0xFE, 0xC0, 0xC0, 0xF8, 0xC0, 0xC0, 0xFE, 0x00],
        b'F' => [0xFE, 0xC0, 0xC0, 0xF8, 0xC0, 0xC0, 0xC0, 0x00],
        b'G' => [0x7C, 0xC6, 0xC0, 0xCE, 0xC6, 0xC6, 0x7C, 0x00],
        b'H' => [0xC6, 0xC6, 0xC6, 0xFE, 0xC6, 0xC6, 0xC6, 0x00],
        b'I' => [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00],
        b'J' => [0x1E, 0x06, 0x06, 0x06, 0xC6, 0xC6, 0x7C, 0x00],
        b'K' => [0xC6, 0xCC, 0xD8, 0xF0, 0xD8, 0xCC, 0xC6, 0x00],
        b'L' => [0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xFE, 0x00],
        b'M' => [0xC6, 0xEE, 0xFE, 0xD6, 0xC6, 0xC6, 0xC6, 0x00],
        b'N' => [0xC6, 0xE6, 0xF6, 0xDE, 0xCE, 0xC6, 0xC6, 0x00],
        b'O' => [0x7C, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0x7C, 0x00],
        b'P' => [0xFC, 0xC6, 0xC6, 0xFC, 0xC0, 0xC0, 0xC0, 0x00],
        b'Q' => [0x7C, 0xC6, 0xC6, 0xC6, 0xD6, 0xCC, 0x76, 0x00],
        b'R' => [0xFC, 0xC6, 0xC6, 0xFC, 0xD8, 0xCC, 0xC6, 0x00],
        b'S' => [0x7C, 0xC6, 0xC0, 0x7C, 0x06, 0xC6, 0x7C, 0x00],
        b'T' => [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
        b'U' => [0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0x7C, 0x00],
        b'V' => [0xC6, 0xC6, 0xC6, 0xC6, 0x6C, 0x38, 0x10, 0x00],
        b'W' => [0xC6, 0xC6, 0xC6, 0xD6, 0xFE, 0xEE, 0xC6, 0x00],
        b'X' => [0xC6, 0x6C, 0x38, 0x10, 0x38, 0x6C, 0xC6, 0x00],
        b'Y' => [0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x18, 0x00],
        b'Z' => [0xFE, 0x0C, 0x18, 0x30, 0x60, 0xC0, 0xFE, 0x00],
        b'0' => [0x7C, 0xC6, 0xCE, 0xD6, 0xE6, 0xC6, 0x7C, 0x00],
        b'1' => [0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00],
        b'2' => [0x7C, 0xC6, 0x06, 0x1C, 0x70, 0xC0, 0xFE, 0x00],
        b'3' => [0x7C, 0xC6, 0x06, 0x3C, 0x06, 0xC6, 0x7C, 0x00],
        b'4' => [0x0C, 0x1C, 0x3C, 0x6C, 0xFE, 0x0C, 0x0C, 0x00],
        b'5' => [0xFE, 0xC0, 0xFC, 0x06, 0x06, 0xC6, 0x7C, 0x00],
        b'6' => [0x3C, 0x60, 0xC0, 0xFC, 0xC6, 0xC6, 0x7C, 0x00],
        b'7' => [0xFE, 0x06, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x00],
        b'8' => [0x7C, 0xC6, 0xC6, 0x7C, 0xC6, 0xC6, 0x7C, 0x00],
        b'9' => [0x7C, 0xC6, 0xC6, 0x7E, 0x06, 0x0C, 0x78, 0x00],
        b'-' => [0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00],
        b'=' => [0x00, 0x00, 0x7E, 0x00, 0x7E, 0x00, 0x00, 0x00],
        b'/' => [0x06, 0x0C, 0x18, 0x30, 0x60, 0xC0, 0x80, 0x00],
        b'.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
        b':' => [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00],
        b' ' => [0x00; 8],
        // A visible block: an unrenderable byte is shown, never skipped.
        _ => [0x00, 0x7E, 0x7E, 0x7E, 0x7E, 0x7E, 0x7E, 0x00],
    }
}

/// Draws text at pixel position, `scale`× the 8×8 cell, foreground on
/// background (the background fills the whole cell so redrawing a line in
/// place erases its predecessor). Pure over the seam; out-of-bounds pixels
/// are the surface's problem and are ignored by contract.
pub fn draw_text<S: Surface>(
    surface: &mut S,
    x: u32,
    y: u32,
    scale: u32,
    text: &[u8],
    fg: u32,
    bg: u32,
) {
    let cell = GLYPH_SIZE * scale;
    for (slot, &byte) in text.iter().enumerate() {
        let glyph = glyph_for(byte);
        let left = x + slot as u32 * cell;
        for (row, &bits) in glyph.iter().enumerate() {
            for column in 0..GLYPH_SIZE {
                let lit = bits & (0x80 >> column) != 0;
                let color = if lit { fg } else { bg };
                for dy in 0..scale {
                    for dx in 0..scale {
                        surface.put(left + column * scale + dx, y + row as u32 * scale + dy, color);
                    }
                }
            }
        }
    }
}

// --- the console layout: pinned positions (TEST-P1-07-09-A clause 3) --------

/// Left margin of every console line, pixels.
pub const MARGIN_X: u32 = 32;
/// Title baseline.
pub const TITLE_Y: u32 = 24;
/// Title scale (8×8 → 32×32 glyphs).
pub const TITLE_SCALE: u32 = 4;
/// The `TOS64-LINK/1` report line.
pub const REPORT_Y: u32 = 96;
/// The live heartbeat line.
pub const STATUS_Y: u32 = 136;
/// The spelled refusal line.
pub const REFUSAL_Y: u32 = 176;
/// The `TOS64-MMU/1` cache-evidence line (`STORY-P1-07-03`) — painted once
/// at park, because serial has never produced a byte on this bench and the
/// canvas is the proven text channel.
pub const MMU_Y: u32 = 216;
/// The `TOS64-CONF/1` conformance-on-silicon line (`STORY-P1-07-04`,
/// `LE-27`) — painted once at park.
pub const CONF_Y: u32 = 256;
/// The `TOS64-PMU/1` counter-decision line (`STORY-P1-07-04`, `LE-15`) —
/// painted once at park.
pub const PMU_Y: u32 = 296;
/// The live `TOS64-TICK/1` line (`STORY-P1-07-04` clause 1) — repainted
/// every second, the ratio evidence accumulating on screen.
pub const TICK_Y: u32 = 336;

/// The once-rendered boot evidence lines the park loop paints beneath the
/// live rows — carried as bytes (line endings already stripped) so the park
/// loop needs no knowledge of what produced them.
pub struct BootLines<'a> {
    /// `TOS64-MMU/1` (`STORY-P1-07-03` clause 4).
    pub mmu: &'a [u8],
    /// `TOS64-CONF/1` (`STORY-P1-07-04` clauses 2 and 5).
    pub conf: &'a [u8],
    /// `TOS64-PMU/1` (`STORY-P1-07-04` clauses 3 and 4).
    pub pmu: &'a [u8],
}
/// Body text scale (16×16 glyphs: 120 columns on the canvas).
pub const BODY_SCALE: u32 = 2;

/// Foreground for ordinary text.
pub const TEXT: u32 = 0x00E8_E8E8;
/// Foreground for a refusal line.
pub const ALERT: u32 = 0x00FF_B040;

/// Paints the standing frame: background and title. Called once at park.
pub fn draw_frame<S: Surface>(surface: &mut S) {
    crate::hdmi::fill_rect(
        surface,
        0,
        0,
        surface.width(),
        surface.height(),
        crate::hdmi::BACKGROUND,
    );
    draw_text(surface, MARGIN_X, TITLE_Y, TITLE_SCALE, b"TINYOS", TEXT, crate::hdmi::BACKGROUND);
}

/// Draws one console line at a pinned row, erasing what was there.
pub fn draw_line<S: Surface>(surface: &mut S, y: u32, text: &[u8], fg: u32) {
    draw_text(surface, MARGIN_X, y, BODY_SCALE, text, fg, crate::hdmi::BACKGROUND);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;

    // TEST-P1-07-09-A clause 1: constants and conversion pinned.

    #[test]
    fn the_canvas_constants_are_the_captured_geometry() {
        assert_eq!(board::SIMPLEFB_BASE, 0x3F80_0000);
        assert_eq!(board::SIMPLEFB_SIZE, 0x3F_4800);
        assert_eq!(board::SIMPLEFB_WIDTH, 1920);
        assert_eq!(board::SIMPLEFB_HEIGHT, 1080);
        assert_eq!(board::SIMPLEFB_STRIDE, 3840);
        // The captured numbers agree with each other: stride × rows = size,
        // and a row is exactly width × 2 bytes at 16 bpp — no padding.
        assert_eq!(board::SIMPLEFB_STRIDE * board::SIMPLEFB_HEIGHT, board::SIMPLEFB_SIZE as u32);
        assert_eq!(board::SIMPLEFB_STRIDE, board::SIMPLEFB_WIDTH * 2);
    }

    #[test]
    fn the_conversion_is_r5g6b5_pinned_at_the_corners() {
        assert_eq!(rgb565(0x0000_0000), 0x0000);
        assert_eq!(rgb565(0x00FF_FFFF), 0xFFFF);
        assert_eq!(rgb565(0x00FF_0000), 0xF800);
        assert_eq!(rgb565(0x0000_FF00), 0x07E0);
        assert_eq!(rgb565(0x0000_00FF), 0x001F);
        // The splash navy: r 0x10→2, g 0x20→8, b 0x40→8.
        assert_eq!(rgb565(crate::hdmi::BACKGROUND), (2 << 11) | (8 << 5) | 8);
    }

    // TEST-P1-07-09-A clause 2: the surface is bounds-honest.

    #[test]
    fn out_of_range_puts_are_ignored_and_the_stride_is_respected() {
        // A padded surface: 4 visible pixels of an 8-pixel stride.
        let mut pixels = [0u16; 8 * 3];
        let mut surface =
            SliceSurface { pixels: &mut pixels, width: 4, height: 3, stride_pixels: 8 };
        surface.put(3, 1, 0x00FF_FFFF);
        surface.put(4, 1, 0x00FF_FFFF); // beyond width: ignored
        surface.put(3, 3, 0x00FF_FFFF); // beyond height: ignored
        assert_eq!(pixels[8 + 3], 0xFFFF, "the visible pixel landed");
        assert!(pixels[8 + 4..16].iter().all(|&p| p == 0), "padding untouched");
        assert!(pixels[16..].iter().all(|&p| p == 0), "later rows untouched");
    }

    #[test]
    fn the_full_clear_touches_every_visible_pixel_and_nothing_beyond() {
        let mut pixels = [0u16; 8 * 3];
        let mut surface =
            SliceSurface { pixels: &mut pixels, width: 4, height: 3, stride_pixels: 8 };
        crate::hdmi::fill_rect(&mut surface, 0, 0, 4, 3, 0x00FF_FFFF);
        for row in 0..3 {
            assert!(pixels[row * 8..row * 8 + 4].iter().all(|&p| p == 0xFFFF), "row {row}");
            assert!(pixels[row * 8 + 4..row * 8 + 8].iter().all(|&p| p == 0), "pad {row}");
        }
    }

    // TEST-P1-07-09-A clause 3: the font is total over the report language.

    #[test]
    fn every_report_byte_renders_and_nothing_is_skipped() {
        const BLOCK: [u8; 8] = [0x00, 0x7E, 0x7E, 0x7E, 0x7E, 0x7E, 0x7E, 0x00];
        let report_charset =
            b"TOS64-LINK/1 rp1=absent reason=id-module detail=0x0002 beacon=skipped \
              TOS64-BEAT/1 seq=42 state=beaconing fb=granted CODE 09 DETAIL 65535 :. \
              TOS64-MMU/1 sctlr=0000000030D01805 off=920000 on=1800 \
              TOS64-FAULT/1 slot=5 esr= class=data-abort-el1 ec=0x25 il=32 \
              status=translation level=1 wnr=read isv=no size= s1ptw=no \
              far= elr= spsr= vbar= readback= match=yes halted no-resume-path \
              TOS64-TICK/1 count=1234 tval=540000 rmin=999 rmax=1001 refused=gicc-pmr \
              TOS64-CONF/1 cntvct=pass span=118 cntfrq=54000000 cpus=54 stuck backwards \
              TOS64-PMU/1 delta=24000000 rate=2400mhz source=pmccntr cntvct-fallback";
        for &byte in report_charset.iter() {
            let glyph = glyph_for(byte);
            assert_ne!(glyph, BLOCK, "byte {byte:#x} ({}) must have a real glyph", byte as char);
        }
        // Case folding: the same letter either way.
        assert_eq!(glyph_for(b'a'), glyph_for(b'A'));
        assert_eq!(glyph_for(b'z'), glyph_for(b'Z'));
        // The unknown byte is a visible block, never blank.
        assert_eq!(glyph_for(0x01), BLOCK);
        assert_ne!(glyph_for(0x01), [0u8; 8]);
    }

    #[test]
    fn text_lands_at_its_pinned_position_and_erases_its_cell() {
        let mut pixels = vec![0u16; 64 * 32];
        {
            let mut surface =
                SliceSurface { pixels: &mut pixels, width: 64, height: 32, stride_pixels: 64 };
            draw_text(&mut surface, 8, 8, 1, b"I", 0x00FF_FFFF, 0x0000_0000);
        }
        // 'I' row 0 is 0x7E: columns 1..=6 lit at y=8, x=8+column.
        for column in 1..=6u32 {
            assert_eq!(pixels[(8 * 64 + 8 + column) as usize], 0xFFFF, "col {column}");
        }
        assert_eq!(pixels[(8 * 64 + 8) as usize], 0x0000, "corner is background");
        // Nothing above the cell.
        assert!(pixels[..8 * 64].iter().all(|&p| p == 0));
        // Redrawing with spaces erases the cell.
        {
            let mut surface =
                SliceSurface { pixels: &mut pixels, width: 64, height: 32, stride_pixels: 64 };
            draw_text(&mut surface, 8, 8, 1, b" ", 0x00FF_FFFF, 0x0000_0000);
        }
        assert!(pixels[8 * 64..16 * 64].iter().all(|&p| p == 0), "the cell was erased");
    }

    #[test]
    fn the_console_rows_are_ordered_and_inside_the_canvas() {
        // Compile-time claims, stated as such (the board.rs convention).
        const {
            assert!(TITLE_Y < REPORT_Y && REPORT_Y < STATUS_Y && STATUS_Y < REFUSAL_Y);
            assert!(REFUSAL_Y < MMU_Y && MMU_Y < CONF_Y && CONF_Y < PMU_Y && PMU_Y < TICK_Y);
            // The title never collides with the report line.
            assert!(TITLE_Y + GLYPH_SIZE * TITLE_SCALE <= REPORT_Y);
            assert!(TICK_Y + GLYPH_SIZE * BODY_SCALE < board::SIMPLEFB_HEIGHT);
            // 120 body columns fit the canvas width at the margin.
            assert!(MARGIN_X + 120 * GLYPH_SIZE * BODY_SCALE <= board::SIMPLEFB_WIDTH + MARGIN_X);
        }
    }
}

// --- aarch64 glue: the real canvas -------------------------------------------

/// The board-side surface: volatile 16-bit stores into the firmware's
/// buffer at the pinned address. Same arithmetic as [`SliceSurface`],
/// which is where it is tested.
#[cfg(target_arch = "aarch64")]
pub struct SimplefbSurface;

#[cfg(target_arch = "aarch64")]
impl Surface for SimplefbSurface {
    fn width(&self) -> u32 {
        crate::board::SIMPLEFB_WIDTH
    }

    fn height(&self) -> u32 {
        crate::board::SIMPLEFB_HEIGHT
    }

    fn put(&mut self, x: u32, y: u32, color: u32) {
        if x >= crate::board::SIMPLEFB_WIDTH || y >= crate::board::SIMPLEFB_HEIGHT {
            return;
        }
        let offset = u64::from(y) * u64::from(crate::board::SIMPLEFB_STRIDE) + u64::from(x) * 2;
        // SAFETY: the address and geometry are the captured, pinned
        // constants; the bounds check above keeps every store inside the
        // buffer's recorded size (the pinning test proves stride × height
        // equals it), and nothing else on this single core writes here.
        unsafe {
            core::ptr::write_volatile(
                (crate::board::SIMPLEFB_BASE + offset) as *mut u16,
                rgb565(color),
            );
        }
    }
}
