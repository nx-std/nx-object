//! BLZ compression for KIP1 segments.
//!
//! BLZ ("backwards LZ") is the LZ77-family scheme that compresses the `text`,
//! `rodata`, and `data` segments of a KIP1 (Kernel Initial Process). The output
//! is a stream of *codes*, each either a literal byte copied verbatim or a
//! back-reference into already-decoded data, grouped under flag bytes and
//! finished with a small trailer.
//!
//! Only compression is implemented here, since that is all the KIP1 builder
//! needs. [`compress`] always allocates a worst-case output buffer up front, so
//! it cannot fail and is infallible for any input (including empty), and it
//! never mutates the caller's slice.
//!
//! # Why "backwards"
//!
//! Both encoding and decoding proceed from the end of the data towards the
//! start. KIP1 segments are decompressed *in place*: the loader expands the
//! compressed bytes into the same memory region. Encoding the tail first means
//! a decoder writing back-to-front never overwrites compressed bytes it has not
//! yet read, so no scratch buffer is required. Consequently the packed region
//! is stored reversed on disk, and a decoder reverses it (and the decoded tail)
//! to recover the original order.
//!
//! # Stream layout
//!
//! A stream takes one of two forms, distinguished by its last four bytes (a
//! little-endian `u32` "extra length"): a value of `0` marks a stored stream,
//! any other value marks a packed stream.
//!
//! ## Stored (incompressible input)
//!
//! When packing would not save space, the bytes are emitted verbatim,
//! zero-padded to a 4-byte boundary, then terminated with a `u32` of `0`:
//!
//! ```text
//! [ raw bytes ][ 0x00 padding ][ u32 = 0 ]
//! ```
//!
//! The decoded result is simply the leading `len - 4` bytes.
//!
//! ## Packed (compressed input)
//!
//! ```text
//! [ raw prefix ][ packed region ][ 0xFF padding ][ enc_len ][ header_size ][ extra_len ]
//! \_ dec_len _/ \___ pak_len __/ \________________ header_size (>= 12) _______________/
//! ```
//!
//! A leading raw prefix that the encoder left unpacked is followed by the
//! reversed packed region and three little-endian `u32` trailer fields. Read
//! from the end of the stream, the fields are:
//!
//! - `extra_len` (final 4 bytes): decompressed bytes produced beyond the
//!   encoded region. Non-zero, which is what distinguishes a packed stream from
//!   a stored one.
//! - `header_size`: size of the three trailer fields plus their `0xFF`
//!   alignment padding (always `>= 12`).
//! - `enc_len`: length of the encoded region — the packed bytes plus the
//!   trailer.
//!
//! From these a decoder recovers the verbatim prefix length
//! (`dec_len = total - enc_len`), the packed byte count
//! (`pak_len = enc_len - header_size`), and the final decompressed length
//! (`dec_len + enc_len + extra_len`).
//!
//! # Code encoding
//!
//! Within the packed region (in decode order, i.e. after a decoder un-reverses
//! it) codes are grouped under *flag bytes*. Each flag byte precedes up to
//! eight codes; its bits are consumed most-significant-first, one per code:
//!
//! - bit `0` — the next byte is a literal, copied as-is.
//! - bit `1` — the next two bytes `b0`, `b1` are a back-reference: the match
//!   length is `(b0 >> 4) + 3` bytes and the distance behind the cursor is
//!   `(((b0 & 0xF) << 8) | b1) + 3`.
//!
//! Match lengths therefore span `3..=18` and distances `3..=0x1002`, the bounds
//! captured by [`MAX_MATCH`] and [`MAX_OFFSET`].
//!
//! # References
//!
//! - <https://switchbrew.org/wiki/KIP>

use std::vec::Vec;

use zerocopy::{IntoBytes, little_endian::U32};

use crate::raw::kip::Blz1Footer;

/// Number of bits the flag mask is shifted between codes.
const FLAG_SHIFT: u8 = 1;
/// Initial flag mask: the top bit of a fresh flag byte.
const FLAG_MASK_INIT: u8 = 0x80;
/// Minimum match length worth encoding as a back-reference.
const MATCH_THRESHOLD: usize = 2;
/// Largest back-reference distance the encoding can represent.
const MAX_OFFSET: usize = 0x1002;
/// Largest match length the encoding can represent.
const MAX_MATCH: usize = (1 << 4) + MATCH_THRESHOLD;

/// Compress `data` with the BLZ algorithm.
///
/// The returned buffer is either a packed stream with a BLZ trailer or, when
/// compression would not save space, the original bytes followed by a trailer
/// marking them as stored. Both forms are accepted by KIP1 segment loaders.
pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut output = vec![0u8; worst_case_len(data.len())];
    // The encoder walks the input back-to-front, so it operates on an owned,
    // reversible copy rather than the caller's borrowed slice.
    let mut input = data.to_vec();
    let len = compress_into(&mut input, &mut output);
    output.truncate(len);
    output
}

/// Upper bound on the compressed size for `raw_len` input bytes.
///
/// One flag bit is spent per emitted code, plus a fixed allowance for the
/// trailer and alignment padding.
fn worst_case_len(raw_len: usize) -> usize {
    raw_len + raw_len.div_ceil(8) + 15
}

/// Encode `input` into `output`, returning the number of bytes written.
///
/// `output` must be at least [`worst_case_len`] bytes; [`compress`] guarantees
/// this, so no bounds failure is possible. `input` is reversed in place during
/// encoding and restored before returning.
fn compress_into(input: &mut [u8], output: &mut [u8]) -> usize {
    input.reverse();

    let len = input.len();
    // Track the split point where (packed bytes + still-raw bytes) is smallest;
    // this is where the stored prefix ends and the packed region begins.
    let mut best_packed = 0;
    let mut best_remaining = len;

    let mut mask = 0u8;
    let mut read_pos = 0;
    let mut write_pos = 0;
    let mut flag_pos = 0;

    while read_pos < len {
        mask >>= FLAG_SHIFT;
        if mask == 0 {
            flag_pos = write_pos;
            output[flag_pos] = 0;
            write_pos += 1;
            mask = FLAG_MASK_INIT;
        }

        let (mut match_len, match_offset) = longest_match(input, read_pos);

        if match_len > MATCH_THRESHOLD && read_pos + match_len < len {
            // Lazy matching: compare emitting this match now against deferring
            // it by one byte, and prefer a literal when deferral packs better.
            read_pos += match_len;
            let (mut next_len, _) = longest_match(input, read_pos);
            read_pos -= match_len - 1;
            let (mut post_len, _) = longest_match(input, read_pos);
            read_pos -= 1;

            if next_len <= MATCH_THRESHOLD {
                next_len = 1;
            }
            if post_len <= MATCH_THRESHOLD {
                post_len = 1;
            }
            if match_len + next_len <= 1 + post_len {
                match_len = 1;
            }
        }

        // Make room for this code's flag bit (set below for a back-reference,
        // left clear for a literal).
        output[flag_pos] <<= 1;
        if match_len > MATCH_THRESHOLD {
            read_pos += match_len;
            output[flag_pos] |= 1;
            // Two-byte back-reference: high nibble of the first byte holds the
            // biased length, the remaining 12 bits hold the biased distance.
            output[write_pos] =
                (((match_len - (MATCH_THRESHOLD + 1)) << 4) | ((match_offset - 3) >> 8)) as u8;
            output[write_pos + 1] = ((match_offset - 3) & 0xFF) as u8;
            write_pos += 2;
        } else {
            output[write_pos] = input[read_pos];
            write_pos += 1;
            read_pos += 1;
        }

        if write_pos + len - read_pos < best_packed + best_remaining {
            best_packed = write_pos;
            best_remaining = len - read_pos;
        }
    }

    // Flush the trailing flag byte by shifting out its unused bits.
    while mask != 0 && mask != 1 {
        mask >>= FLAG_SHIFT;
        output[flag_pos] <<= 1;
    }

    let packed_len = write_pos;

    input.reverse();
    output[..packed_len].reverse();

    // Compare the aligned packed layout against simply storing the raw bytes.
    if best_packed == 0 || len + 4 < ((best_packed + best_remaining + 3) & 0xFFFF_FFFC) + 8 {
        store_uncompressed(input, output)
    } else {
        store_packed(input, output, packed_len, best_packed, best_remaining)
    }
}

/// Emit `input` verbatim with a zero trailer marking the data as stored.
fn store_uncompressed(input: &[u8], output: &mut [u8]) -> usize {
    let len = input.len();
    output[..len].copy_from_slice(input);

    let mut pos = len;
    while pos & 3 != 0 {
        output[pos] = 0;
        pos += 1;
    }

    output[pos..pos + 4].copy_from_slice(U32::new(0).as_bytes());
    pos + 4
}

/// Assemble the final stream: a raw prefix, the packed region relocated behind
/// it, and a 12-byte (plus alignment) trailer describing the layout.
///
/// `packed_len` is the full length of the encoded stream; only its last
/// `best_packed` bytes (the optimal split) are retained, placed after the
/// `remaining` raw prefix bytes.
fn store_packed(
    input: &[u8],
    output: &mut [u8],
    packed_len: usize,
    best_packed: usize,
    remaining: usize,
) -> usize {
    // Relocate the packed bytes past the raw prefix before copying the prefix
    // in, so the prefix copy cannot clobber not-yet-moved packed data.
    let packed_start = packed_len - best_packed;
    for i in 0..best_packed {
        output[remaining + i] = output[packed_start + i];
    }
    output[..remaining].copy_from_slice(&input[..remaining]);

    let mut pos = remaining + best_packed;
    let mut header_size = 12;
    let inc_len = input.len() - best_packed - remaining;

    // Pad with 0xFF so the trailer ends on a 4-byte boundary; the padding
    // counts towards the recorded header size.
    while pos & 3 != 0 {
        output[pos] = 0xFF;
        pos += 1;
        header_size += 1;
    }

    // Trailer (see module docs): enc_len, header_size, extra_len.
    let footer = Blz1Footer {
        enc_len: U32::new((best_packed + header_size) as u32),
        header_size: U32::new(header_size as u32),
        extra_len: U32::new((inc_len - header_size) as u32),
    };
    output[pos..pos + size_of::<Blz1Footer>()].copy_from_slice(footer.as_bytes());
    pos + size_of::<Blz1Footer>()
}

/// Find the longest back-reference for the data starting at `pos`.
///
/// Returns the match length and its distance behind `pos`. When no match
/// exceeds [`MATCH_THRESHOLD`], the length is [`MATCH_THRESHOLD`] and the
/// offset is `0`, signalling the caller to emit a literal.
fn longest_match(data: &[u8], pos: usize) -> (usize, usize) {
    let mut best_len = MATCH_THRESHOLD;
    let mut best_offset = 0;
    let max_offset = pos.min(MAX_OFFSET);

    for offset in 3..=max_offset {
        let mut len = 0;
        while len < MAX_MATCH {
            if len == data.len() - pos || len >= offset {
                break;
            }
            if data[pos + len] != data[pos + len - offset] {
                break;
            }
            len += 1;
        }

        if len > best_len {
            best_offset = offset;
            best_len = len;
            if best_len == MAX_MATCH {
                break;
            }
        }
    }

    (best_len, best_offset)
}
