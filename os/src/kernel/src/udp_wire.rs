//! The same spoor records, wrapped so an ordinary socket can read them
//! (`STORY-P1-10-03`).
//!
//! # Why this exists
//!
//! The canonical spoor frame is raw EtherType `0x88B5`
//! ([`crate::spoor_wire`]) and stays that way. But Windows demultiplexes
//! incoming frames by EtherType inside NDIS, in the kernel, and discards
//! anything with no registered protocol driver — so `0x88B5` never reaches
//! user mode at *any* privilege level. Reading it needs a signed kernel
//! driver, which is a large prerequisite to put in front of every future
//! diagnostic session on every future machine.
//!
//! Wrapping the identical payload in IPv4/UDP costs 28 bytes of almost
//! entirely constant header and removes that prerequisite completely: the
//! host's own IP stack delivers it to an unprivileged socket.
//!
//! # What it does not cost
//!
//! **No attack surface.** Attack surface is a property of what the board
//! *parses*, and the board parses nothing — receive is disabled and
//! `gem.rs` enforces that with a test. This module only writes bytes
//! outward. The minimal-surface argument for the raw format is untouched,
//! because it was never an argument about egress.
//!
//! **No IP stack.** There is no ARP, no routing, no fragmentation, no
//! connection and no state. The destination is the broadcast address, which
//! needs no address resolution, and every header field below is either a
//! constant or a length.
//!
//! # One field that is a safety property
//!
//! `TTL` is **1**. These frames can be delivered on the local segment and can
//! never be forwarded off it by any conforming router. A diagnostic stream
//! with no confidentiality and no authenticity should not be able to leave the
//! cable it was meant for, and one byte enforces that better than a policy.

/// The UDP port spoor frames are broadcast to.
pub const SPOOR_UDP_PORT: u16 = 6404;

/// IPv4 header bytes (no options — this module never emits any).
pub const IPV4_HEADER_LEN: usize = 20;

/// UDP header bytes.
pub const UDP_HEADER_LEN: usize = 8;

/// Total bytes prepended to a payload.
pub const HEADER_LEN: usize = IPV4_HEADER_LEN + UDP_HEADER_LEN;

/// IANA protocol number for UDP.
const PROTOCOL_UDP: u8 = 17;

/// Hop limit of one: deliverable on this segment, never forwarded off it.
const TTL_LINK_LOCAL: u8 = 1;

/// Why a datagram could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdpWireError {
    /// The output buffer cannot hold the headers plus the payload.
    BufferTooSmall,
    /// The payload would push the datagram past what a `u16` length can state.
    PayloadTooLarge,
}

/// The one's-complement checksum IPv4 requires over its own header.
///
/// Pure and host-testable, which matters because it is the only field here a
/// receiver actually validates: a wrong checksum means the host's IP stack
/// drops the datagram silently and the stream simply never appears, with
/// nothing anywhere saying why.
#[must_use]
pub fn ipv4_checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut index = 0;
    while index + 1 < header.len() {
        sum += u32::from(u16::from_be_bytes([header[index], header[index + 1]]));
        index += 2;
    }
    if index < header.len() {
        sum += u32::from(header[index]) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// Wraps `payload` in IPv4/UDP broadcast headers, returning the total length.
///
/// The UDP checksum is written as zero, which IPv4 explicitly permits and
/// which every conforming receiver accepts — the payload's own integrity is
/// not this layer's job, and a wrong checksum here would cost a silent drop
/// for no benefit.
///
/// # Errors
///
/// [`UdpWireError::BufferTooSmall`] if `out` cannot hold the datagram, or
/// [`UdpWireError::PayloadTooLarge`] if the total would exceed a `u16` length
/// field. Nothing is written in either case.
pub fn encode(payload: &[u8], out: &mut [u8]) -> Result<usize, UdpWireError> {
    let total = HEADER_LEN + payload.len();
    if total > u16::MAX as usize {
        return Err(UdpWireError::PayloadTooLarge);
    }
    if out.len() < total {
        return Err(UdpWireError::BufferTooSmall);
    }

    let udp_len = (UDP_HEADER_LEN + payload.len()) as u16;

    out[0] = 0x45; // IPv4, header length 5 words — no options, ever.
    out[1] = 0; // DSCP/ECN: default.
    out[2..4].copy_from_slice(&(total as u16).to_be_bytes());
    out[4..6].copy_from_slice(&0u16.to_be_bytes()); // identification
    out[6..8].copy_from_slice(&0u16.to_be_bytes()); // flags/fragment: neither
    out[8] = TTL_LINK_LOCAL;
    out[9] = PROTOCOL_UDP;
    out[10..12].copy_from_slice(&0u16.to_be_bytes()); // checksum, filled below
    out[12..16].copy_from_slice(&[0, 0, 0, 0]); // source 0.0.0.0 — the board has no address
    out[16..20].copy_from_slice(&[255, 255, 255, 255]); // limited broadcast

    let checksum = ipv4_checksum(&out[..IPV4_HEADER_LEN]);
    out[10..12].copy_from_slice(&checksum.to_be_bytes());

    let udp = IPV4_HEADER_LEN;
    out[udp..udp + 2].copy_from_slice(&SPOOR_UDP_PORT.to_be_bytes());
    out[udp + 2..udp + 4].copy_from_slice(&SPOOR_UDP_PORT.to_be_bytes());
    out[udp + 4..udp + 6].copy_from_slice(&udp_len.to_be_bytes());
    out[udp + 6..udp + 8].copy_from_slice(&0u16.to_be_bytes()); // checksum: none

    out[HEADER_LEN..total].copy_from_slice(payload);
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A checksum is correct exactly when the receiver's sum over the header
    /// including it comes to zero. Checking that property rather than a
    /// hand-computed constant is what makes this a test of the algorithm.
    #[test]
    fn the_header_checksum_validates_the_way_a_receiver_validates_it() {
        let mut out = [0u8; 128];
        let len = encode(b"payload", &mut out).expect("fits");
        assert!(len > HEADER_LEN);
        assert_eq!(
            ipv4_checksum(&out[..IPV4_HEADER_LEN]),
            0,
            "summing a correct header including its checksum must yield zero"
        );
    }

    /// Known-answer check against the standard worked IPv4 example, so the
    /// implementation is held to an outside answer and not merely to itself.
    ///
    /// Header: `4500 0073 0000 4000 4011 ---- c0a8 0001 c0a8 00c7`
    /// (total 0x73, DF set, TTL 64, proto UDP, 192.168.0.1 → 192.168.0.199),
    /// whose published checksum is `0xB861`. Verified by hand rather than
    /// trusted: the one's-complement sum of the words with the checksum field
    /// zeroed folds to `0x479E`, and `!0x479E == 0xB861`.
    #[test]
    fn the_checksum_matches_an_independently_computed_header() {
        let header = [
            0x45u8, 0x00, 0x00, 0x73, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0x00, 0x00, 0xC0, 0xA8,
            0x00, 0x01, 0xC0, 0xA8, 0x00, 0xC7,
        ];
        assert_eq!(ipv4_checksum(&header), 0xB861);

        // And the receiver-side property: with the checksum in place, the sum
        // over the whole header must come to zero.
        let mut checked = header;
        checked[10..12].copy_from_slice(&0xB861u16.to_be_bytes());
        assert_eq!(ipv4_checksum(&checked), 0);
    }

    /// One byte that is a policy: a stream with no confidentiality and no
    /// authenticity must not be able to leave the cable it was meant for.
    #[test]
    fn the_datagram_cannot_be_routed_off_the_local_segment() {
        let mut out = [0u8; 128];
        encode(b"x", &mut out).expect("fits");
        assert_eq!(out[8], 1, "TTL of 1 is never forwarded by a conforming router");
        assert_eq!(&out[16..20], &[255, 255, 255, 255], "limited broadcast, never a unicast route");
    }

    #[test]
    fn there_is_never_a_fragment_and_never_an_option() {
        let mut out = [0u8; 128];
        encode(&[0u8; 64], &mut out).expect("fits");
        assert_eq!(out[0], 0x45, "IHL 5: no options region exists to walk");
        assert_eq!(&out[6..8], &[0, 0], "no flags, no fragment offset");
    }

    #[test]
    fn the_lengths_agree_with_each_other_and_with_the_payload() {
        let payload = [0xABu8; 100];
        let mut out = [0u8; 256];
        let total = encode(&payload, &mut out).expect("fits");
        assert_eq!(total, HEADER_LEN + payload.len());
        assert_eq!(u16::from_be_bytes([out[2], out[3]]), total as u16, "IP total length");
        assert_eq!(
            u16::from_be_bytes([out[24], out[25]]),
            (UDP_HEADER_LEN + payload.len()) as u16,
            "UDP length"
        );
        assert_eq!(&out[HEADER_LEN..total], &payload, "the payload is carried verbatim");
    }

    #[test]
    fn the_payload_is_the_spoor_frame_unchanged() {
        // The whole point: UDP carries the identical records, so one decoder
        // reads a raw capture, a UDP datagram and a journal file alike.
        let mut spoor = [0u8; crate::spoor_wire::MAX_PAYLOAD];
        let spoor_len = crate::spoor_wire::encode(7, &[0x1234_5678_9ABC_DEF0], &mut spoor)
            .expect("a spoor frame encodes");
        let mut out = [0u8; 2048];
        let total = encode(&spoor[..spoor_len], &mut out).expect("fits");
        assert_eq!(&out[HEADER_LEN..total], &spoor[..spoor_len]);
        assert_eq!(
            crate::spoor_wire::decode_header(&out[HEADER_LEN..total]).expect("decodes"),
            (7, 1)
        );
    }

    #[test]
    fn both_ports_are_the_spoor_port() {
        let mut out = [0u8; 64];
        encode(b"", &mut out).expect("fits");
        assert_eq!(u16::from_be_bytes([out[20], out[21]]), SPOOR_UDP_PORT);
        assert_eq!(u16::from_be_bytes([out[22], out[23]]), SPOOR_UDP_PORT);
    }

    #[test]
    fn a_buffer_too_small_is_refused_before_a_byte_is_written() {
        let mut out = [0u8; HEADER_LEN];
        assert_eq!(encode(b"too long", &mut out), Err(UdpWireError::BufferTooSmall));
        assert!(out.iter().all(|b| *b == 0), "nothing written on refusal");
    }
}
