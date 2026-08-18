//! BLZ compression for KIP1 segments.
//!
//! BLZ ("backwards LZ") is the LZ77-family scheme that compresses the `text`,
//! `rodata`, and `data` segments of a KIP1 (Kernel Initial Process). The output
//! is a stream of *codes*, each either a literal byte copied verbatim or a
//! back-reference into already-decoded data, grouped under flag bytes and
//! finished with a small trailer.
//!
//! [`compress`] always allocates a worst-case output buffer up front, so it
//! cannot fail and is infallible for any input (including empty), and it never
//! mutates the caller's slice. [`decompress`] is its inverse and is fallible,
//! because it is the half that reads bytes the crate did not produce.
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
//! The decoded result is the leading bytes, but *how many* is not recorded anywhere in the
//! stream: the padding is indistinguishable from data. A decoder is told the length from
//! outside — for a KIP1 segment it is the segment header's `decomp_size`.
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
//! - <https://switchbrew.org/wiki/KIP1>

use alloc::{vec, vec::Vec};

use static_assertions::const_assert_eq;
use zerocopy::{IntoBytes, little_endian::U32};

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
/// Shortest match the encoding represents and the prefix width the match finder
/// hashes; it equals the offset bias, so it is also the smallest representable
/// back-reference distance.
const MIN_MATCH: usize = MATCH_THRESHOLD + 1;
/// Number of match-finder hash buckets, as a power-of-two exponent.
const HASH_BITS: u32 = 16;
/// Number of match-finder hash buckets.
const HASH_SIZE: usize = 1 << HASH_BITS;
/// Fibonacci-hashing multiplier (≈2³² / φ) that spreads prefixes across buckets.
const HASH_MULTIPLIER: u32 = 0x9E37_79B1;
/// Empty-bucket and end-of-chain marker for the match finder.
const NONE: u32 = u32::MAX;

// The match finder hashes a fixed three-byte prefix, so MIN_MATCH must stay 3.
const_assert_eq!(MIN_MATCH, 3);

/// Trailer of a BLZ-packed stream.
///
/// Written at the very end of a compressed stream, it lets a decoder recover
/// the layout while decompressing in place. The fields are little-endian and
/// read from the end of the stream as `extra_len`, then `header_size`, then
/// `enc_len`.
#[derive(Debug, Clone, Copy, zerocopy::IntoBytes, zerocopy::Immutable)]
#[repr(C)]
struct Footer {
    /// Length of the encoded region: packed bytes plus this trailer.
    enc_len: U32,
    /// Size of the trailer plus its `0xFF` alignment padding (always `>= 12`).
    header_size: U32,
    /// Decompressed bytes produced beyond the encoded region. Non-zero, which
    /// distinguishes a packed stream from a stored one (whose trailer is `0`).
    extra_len: U32,
}

// The trailer layout is fixed at 12 bytes (three little-endian u32s).
const_assert_eq!(size_of::<Footer>(), 12);

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
    let finder = MatchFinder::new(input);
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

        let (mut match_len, match_offset) = finder.longest_match(read_pos);

        if match_len > MATCH_THRESHOLD && read_pos + match_len < len {
            // Lazy matching: compare emitting this match now against deferring
            // it by one byte, and prefer a literal when deferral packs better.
            read_pos += match_len;
            let (mut next_len, _) = finder.longest_match(read_pos);
            read_pos -= match_len - 1;
            let (mut post_len, _) = finder.longest_match(read_pos);
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

    // The trailer is written into the bytes the packed layout saves, so a layout saving less than
    // the trailer costs cannot be expressed: `extra_len` would have to be negative. Saving exactly
    // the trailer is no better — that writes `extra_len` as zero, which is the marker for a stored
    // stream, and the result would decode as one. Both cases fall back to storing, which is valid
    // and, for the small highly compressible inputs that reach here, smaller anyway.
    let trailer_fits =
        len - best_packed - best_remaining > packed_header_size(best_remaining + best_packed);

    // Compare the aligned packed layout against simply storing the raw bytes.
    if best_packed == 0
        || !trailer_fits
        || len + 4 < ((best_packed + best_remaining + 3) & 0xFFFF_FFFC) + 8
    {
        store_uncompressed(input, output)
    } else {
        store_packed(input, output, packed_len, best_packed, best_remaining)
    }
}

/// Size of the trailer for a packed region ending at `packed_end`, alignment padding included.
///
/// The padding brings the trailer onto a 4-byte boundary and is counted in the recorded header
/// size, so both the layout decision and the writer have to agree on it.
fn packed_header_size(packed_end: usize) -> usize {
    size_of::<Footer>() + (packed_end.wrapping_neg() & 3)
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
    let header_size = packed_header_size(pos);
    let inc_len = input.len() - best_packed - remaining;

    // Pad with 0xFF so the trailer ends on a 4-byte boundary; the padding
    // counts towards the recorded header size.
    while pos & 3 != 0 {
        output[pos] = 0xFF;
        pos += 1;
    }

    // Trailer (see module docs): enc_len, header_size, extra_len.
    let footer = Footer {
        enc_len: U32::new((best_packed + header_size) as u32),
        header_size: U32::new(header_size as u32),
        extra_len: U32::new((inc_len - header_size) as u32),
    };
    output[pos..pos + size_of::<Footer>()].copy_from_slice(footer.as_bytes());
    pos + size_of::<Footer>()
}

/// Hash-chain index over the reversed input, used to find back-references
/// without scanning every candidate offset.
///
/// The chain is built over the whole buffer up front so that any query position
/// — including the forward look-ahead positions the lazy matcher probes —
/// already has its full chain of earlier same-prefix positions available. Each
/// `prev` link only ever points to an earlier position, so a fully built table
/// answers every query correctly regardless of the order the encoder visits
/// positions in.
struct MatchFinder<'a> {
    /// Reversed input the matcher searches over.
    data: &'a [u8],
    /// For each position, the previous position sharing its [`MIN_MATCH`]-byte
    /// hash bucket, forming a newest-to-oldest chain terminated by [`NONE`].
    prev: Vec<u32>,
}

impl<'a> MatchFinder<'a> {
    /// Build the hash-chain index over `data` in a single forward pass.
    ///
    /// # Panics
    ///
    /// Panics if `data` is longer than [`NONE`], which no KIP1 segment can be:
    /// positions are stored as `u32`, and a buffer that long would alias the
    /// end-of-chain sentinel.
    fn new(data: &'a [u8]) -> Self {
        assert!(data.len() < NONE as usize, "input too large to index");

        // `head` maps each bucket to its most recent position and is only needed
        // while linking the chain, so it stays local to construction.
        let mut head = vec![NONE; HASH_SIZE];
        let mut prev = vec![NONE; data.len()];

        // Link each hashable position to the previous occupant of its bucket, so
        // `prev[pos]` becomes the nearest earlier same-prefix position. Only
        // positions with a full three-byte prefix are hashable.
        if let Some(hashable) = data.len().checked_sub(MIN_MATCH - 1) {
            for (pos, slot) in prev.iter_mut().enumerate().take(hashable) {
                let bucket = Self::hash(data, pos);
                *slot = head[bucket];
                // `pos` indexes `data`, whose length the assert above pins below
                // the `NONE` sentinel, so this can neither truncate nor collide
                // with it.
                head[bucket] = pos as u32;
            }
        }

        Self { data, prev }
    }

    /// Find the longest back-reference for the data starting at `pos`.
    ///
    /// Returns the match length and its distance behind `pos`. When no match
    /// exceeds [`MATCH_THRESHOLD`], the length is [`MATCH_THRESHOLD`] and the
    /// offset is `0`, signalling the caller to emit a literal. Walking the chain
    /// nearest-first and keeping only strictly longer matches selects the
    /// longest match and, among equal lengths, the smallest offset — exactly as
    /// an ascending offset scan would.
    fn longest_match(&self, pos: usize) -> (usize, usize) {
        let data = self.data;

        // Fewer than MIN_MATCH bytes remain: no encodable match starts here.
        if data.len() < pos + MIN_MATCH {
            return (MATCH_THRESHOLD, 0);
        }

        let remaining = data.len() - pos;
        let mut best_len = MATCH_THRESHOLD;
        let mut best_offset = 0;

        let mut candidate = self.prev[pos];
        while candidate != NONE {
            let candidate_pos = candidate as usize;
            let offset = pos - candidate_pos;
            if offset > MAX_OFFSET {
                break;
            }

            // Offsets below MIN_MATCH cannot be encoded; skip them without
            // ending the walk, since nearer links may already be that close.
            if offset >= MIN_MATCH {
                let mut len = 0;
                while len < MAX_MATCH
                    && len != remaining
                    && len < offset
                    && data[pos + len] == data[pos + len - offset]
                {
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

            candidate = self.prev[candidate_pos];
        }

        (best_len, best_offset)
    }

    /// Map the three-byte prefix at `pos` to a hash bucket.
    fn hash(data: &[u8], pos: usize) -> usize {
        let key = u32::from(data[pos])
            | (u32::from(data[pos + 1]) << 8)
            | (u32::from(data[pos + 2]) << 16);
        (key.wrapping_mul(HASH_MULTIPLIER) >> (u32::BITS - HASH_BITS)) as usize
    }
}

/// Decompress a BLZ stream produced by [`compress`] into `decompressed_size` bytes.
///
/// The size is a parameter because a stream does not carry it. A stored stream is padded to a
/// 4-byte boundary and the padding is indistinguishable from data, so nothing in the bytes says
/// where the original ended; a caller reading a KIP1 takes the length from the segment header's
/// `decomp_size`, which is what the loader does too.
///
/// # Errors
///
/// Returns an error if the buffer is too short to hold a trailer, if the trailer's lengths do not
/// describe a layout that fits the buffer, if a code reaches outside the region it may read, or if
/// the stream does not hold `decompressed_size` bytes of output.
pub fn decompress(data: &[u8], decompressed_size: usize) -> Result<Vec<u8>, DecompressError> {
    let extra_len = trailing_u32(data, 4).ok_or(DecompressError::BufferTooSmall {
        required: 4,
        available: data.len(),
    })?;

    // A zero here is what marks a stored stream: the result is its leading bytes, and everything
    // after them up to the trailer is alignment padding the caller's length tells us to drop.
    if extra_len == 0 {
        let stored = &data[..data.len() - 4];
        return stored.get(..decompressed_size).map(<[u8]>::to_vec).ok_or(
            DecompressError::UnexpectedLength {
                expected: decompressed_size,
                produced: stored.len(),
            },
        );
    }

    let footer_size = size_of::<Footer>();
    let header_size = trailing_u32(data, 8).ok_or(DecompressError::BufferTooSmall {
        required: footer_size,
        available: data.len(),
    })? as usize;
    let enc_len = trailing_u32(data, footer_size).ok_or(DecompressError::BufferTooSmall {
        required: footer_size,
        available: data.len(),
    })? as usize;

    let total = data.len();
    if header_size < footer_size || header_size > enc_len || enc_len > total {
        return Err(DecompressError::MalformedTrailer {
            enc_len,
            header_size,
            available: total,
        });
    }

    // The final length is `dec_len + enc_len + extra_len`, and `dec_len` is `total - enc_len`.
    let final_len =
        total
            .checked_add(extra_len as usize)
            .ok_or(DecompressError::MalformedTrailer {
                enc_len,
                header_size,
                available: total,
            })?;

    // Decoding runs in place, exactly as the loader does it: the stream is copied into a buffer
    // sized for the result, then the packed region is consumed backwards while the output cursor
    // walks down from the end. The two never cross, which is what lets the scheme work with no
    // scratch buffer — and it is why the encoder reversed the region in the first place.
    let mut out = vec![0u8; final_len];
    out[..total].copy_from_slice(data);

    let packed_start = total - enc_len;
    let mut cmp = total - header_size;
    let mut dst = final_len;

    while cmp > packed_start {
        cmp -= 1;
        let mut flags = out[cmp];

        for _ in 0..8 {
            if cmp <= packed_start {
                break;
            }

            if flags & FLAG_MASK_INIT == 0 {
                cmp -= 1;
                dst = copy_literal(&mut out, cmp, dst)?;
            } else {
                if cmp - packed_start < 2 {
                    return Err(DecompressError::TruncatedCode { at: cmp });
                }
                let first = out[cmp - 1];
                let second = out[cmp - 2];
                cmp -= 2;

                let length = usize::from(first >> 4) + MIN_MATCH;
                let distance = ((usize::from(first & 0x0F) << 8) | usize::from(second)) + MIN_MATCH;
                dst = copy_match(&mut out, dst, length, distance)?;
            }

            flags <<= FLAG_SHIFT;
        }
    }

    if out.len() != decompressed_size {
        return Err(DecompressError::UnexpectedLength {
            expected: decompressed_size,
            produced: out.len(),
        });
    }

    Ok(out)
}

/// Copy one literal byte from the packed region to the output cursor.
fn copy_literal(out: &mut [u8], cmp: usize, dst: usize) -> Result<usize, DecompressError> {
    let dst = dst.checked_sub(1).ok_or(DecompressError::OutputUnderflow)?;
    out[dst] = out[cmp];
    Ok(dst)
}

/// Copy `length` bytes from `distance` ahead of the output cursor, one byte at a time.
///
/// Byte by byte rather than as a block: a match may overlap the region it writes into, which is how
/// a run is encoded, and a block copy would read bytes this call has not produced yet.
fn copy_match(
    out: &mut [u8],
    mut dst: usize,
    length: usize,
    distance: usize,
) -> Result<usize, DecompressError> {
    for _ in 0..length {
        dst = dst.checked_sub(1).ok_or(DecompressError::OutputUnderflow)?;
        let from = dst
            .checked_add(distance)
            .filter(|from| *from < out.len())
            .ok_or(DecompressError::MatchOutOfBounds { dst, distance })?;
        out[dst] = out[from];
    }

    Ok(dst)
}

/// The little-endian `u32` sitting `back` bytes from the end of `data`.
fn trailing_u32(data: &[u8], back: usize) -> Option<u32> {
    let start = data.len().checked_sub(back)?;
    let bytes: [u8; 4] = data.get(start..start + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

/// Error returned by [`decompress`].
#[derive(Debug, thiserror::Error)]
pub enum DecompressError {
    /// The buffer is too short to hold the trailer a stream ends with.
    #[error("buffer too small: need {required} bytes, have {available}")]
    BufferTooSmall {
        /// Number of bytes required.
        required: usize,
        /// Number of bytes available.
        available: usize,
    },
    /// The trailer's lengths do not describe a layout that fits the buffer.
    ///
    /// Holds the three quantities that disagree.
    #[error(
        "trailer claims {enc_len} encoded bytes under a {header_size}-byte header, in {available}"
    )]
    MalformedTrailer {
        /// Length of the encoded region the trailer records.
        enc_len: usize,
        /// Size of the trailer and its padding, as recorded.
        header_size: usize,
        /// Number of bytes the stream actually holds.
        available: usize,
    },
    /// A back-reference was cut short by the start of the packed region.
    ///
    /// Holds where in the stream the truncated code began.
    #[error("a back-reference at {at} runs past the start of the packed region")]
    TruncatedCode {
        /// Offset the truncated code began at.
        at: usize,
    },
    /// A code would write before the start of the output.
    ///
    /// The stream encodes more output than its trailer accounts for.
    #[error("the stream decodes to more bytes than its trailer records")]
    OutputUnderflow,
    /// The stream does not hold the number of output bytes the caller asked for.
    ///
    /// Holds both lengths. For a KIP1 segment this means the header's `decomp_size` and the stream
    /// disagree, so one of the two was rewritten without the other.
    #[error("expected {expected} decompressed bytes, the stream holds {produced}")]
    UnexpectedLength {
        /// Length the caller asked for.
        expected: usize,
        /// Length the stream actually decodes to.
        produced: usize,
    },
    /// A back-reference points past what has been decoded.
    ///
    /// Holds the write cursor and the distance that overshot it.
    #[error("a back-reference at {dst} reaches {distance} bytes ahead, past the decoded output")]
    MatchOutOfBounds {
        /// Position being written.
        dst: usize,
        /// Distance the reference reached.
        distance: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Brute-force search retained as the oracle the hash-chain finder must
    /// match. This is the original implementation: it scans every offset, so
    /// `MatchFinder::longest_match` is required to return an identical result.
    fn longest_match_brute_force(data: &[u8], pos: usize) -> (usize, usize) {
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

    /// Deterministic pseudo-random bytes (xorshift64) for an incompressible
    /// corpus without pulling in an RNG dependency.
    fn pseudo_random_bytes(len: usize, seed: u64) -> Vec<u8> {
        let mut state = seed;
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state & 0xFF) as u8
            })
            .collect()
    }

    /// Collect [`MatchFinder::longest_match`] at every position of the indexed
    /// buffer.
    fn longest_match_at_every_position(finder: &MatchFinder<'_>) -> Vec<(usize, usize)> {
        (0..finder.data.len())
            .map(|pos| finder.longest_match(pos))
            .collect()
    }

    /// Assert `matches` holds the brute-force result for every position of
    /// `data`.
    ///
    /// `compress`'s output is a deterministic function of these per-position
    /// results, so agreement at every position guarantees `compress` produces a
    /// byte-identical stream — a stronger and more diagnostic check than
    /// comparing the final buffers.
    fn assert_matches_brute_force(data: &[u8], matches: &[(usize, usize)]) {
        assert_eq!(
            matches.len(),
            data.len(),
            "should hold one match per input position"
        );
        for (pos, actual) in matches.iter().enumerate() {
            let expected = longest_match_brute_force(data, pos);
            assert_eq!(
                *actual, expected,
                "finder diverged from brute force at position {pos}"
            );
        }
    }

    /// Assert a stream round-trips: what `compress` produced must decode back to `data`.
    ///
    /// This is the whole contract between the two halves, so every corpus below asserts it rather
    /// than inspecting the encoded bytes — those are an implementation detail, the round trip is not.
    fn assert_round_trips(data: &[u8]) {
        let compressed = compress(data);
        let decompressed =
            decompress(&compressed, data.len()).expect("a stream we produced should decode");
        assert_eq!(
            decompressed,
            data,
            "round trip diverged for a {}-byte input",
            data.len()
        );
    }

    #[test]
    fn compress_with_a_short_run_stores_rather_than_packing() {
        //* Given
        // Twelve identical bytes pack into less than the trailer costs. Choosing the packed layout
        // here once underflowed `extra_len`; the fallback is what keeps the layout expressible.
        let data = vec![0x11u8; 12];

        //* When
        let compressed = compress(&data);

        //* Then
        let extra_len = u32::from_le_bytes(
            compressed[compressed.len() - 4..]
                .try_into()
                .expect("a 4-byte tail converts into [u8; 4]"),
        );
        assert_eq!(extra_len, 0, "the stream should be marked stored");
        assert_eq!(
            decompress(&compressed, data.len()).expect("a stored stream should decode"),
            data
        );
    }

    #[test]
    fn compress_never_writes_a_zero_extra_length_on_a_packed_stream() {
        //* Given
        // Zero is the stored marker, so a packed stream carrying it decodes as the wrong form.
        // The lengths that saved exactly the trailer size used to produce one.
        let corpus = pseudo_random_bytes(600, 0x51ED_5EED_C0FF_EE01);

        //* When
        let streams: Vec<Vec<u8>> = (0..600)
            .flat_map(|len| [compress(&vec![0x22u8; len]), compress(&corpus[..len])])
            .collect();

        //* Then
        for (index, stream) in streams.iter().enumerate() {
            let extra_len = u32::from_le_bytes(
                stream[stream.len() - 4..]
                    .try_into()
                    .expect("a 4-byte tail converts into [u8; 4]"),
            );
            let stored_len = stream.len() - 4;
            assert!(
                extra_len != 0 || stored_len >= index / 2,
                "stream {index} claims stored but is too short to hold its input"
            );
        }
    }

    #[test]
    fn decompress_with_empty_input_returns_empty() {
        //* Given
        let data: Vec<u8> = Vec::new();

        //* When
        let compressed = compress(&data);

        //* Then
        assert_eq!(
            decompress(&compressed, data.len()).expect("an empty stream should decode"),
            data
        );
    }

    #[test]
    fn decompress_with_incompressible_input_round_trips() {
        //* Given
        // Random bytes take the stored path, which is the branch a packed decoder never reaches.
        let data = pseudo_random_bytes(4096, 0x2545_F491_4F6C_DD1D);

        //* When
        let compressed = compress(&data);

        //* Then
        assert_eq!(
            decompress(&compressed, data.len()).expect("a stored stream should decode"),
            data
        );
    }

    #[test]
    fn decompress_with_rle_input_round_trips() {
        //* Given
        // A single run is the densest possible packing: every code is a maximal back-reference.
        let data = vec![0xAAu8; 8192];

        //* When
        let compressed = compress(&data);

        //* Then
        assert_eq!(
            decompress(&compressed, data.len()).expect("a packed stream should decode"),
            data
        );
    }

    #[test]
    fn decompress_with_structured_input_round_trips() {
        //* Given
        // Literals and matches interleaved, so flag bytes carry a mix of both kinds of code.
        let mut data = Vec::new();
        for index in 0..512u32 {
            data.extend_from_slice(&index.to_le_bytes());
            data.extend_from_slice(b"nx-object");
        }

        //* When
        let compressed = compress(&data);

        //* Then
        assert_eq!(
            decompress(&compressed, data.len()).expect("a packed stream should decode"),
            data
        );
    }

    #[test]
    fn decompress_with_a_prefix_that_stays_raw_round_trips() {
        //* Given
        // Incompressible head, highly compressible tail: the encoder leaves a verbatim prefix, so
        // the decoder must place the packed region after it rather than at offset zero.
        let mut data = pseudo_random_bytes(2048, 0x9E37_79B9_7F4A_7C15);
        data.extend_from_slice(&vec![0x5Cu8; 8192]);

        //* When
        let compressed = compress(&data);

        //* Then
        assert_eq!(
            decompress(&compressed, data.len()).expect("a packed stream should decode"),
            data
        );
    }

    #[test]
    fn decompress_with_lengths_from_one_to_a_thousand_round_trips() {
        //* Given
        // Sweeps the boundaries: empty, shorter than a match, and every alignment of the trailer
        // padding and the eight-code flag group.
        let corpus = pseudo_random_bytes(1000, 0x0DDB_1A5E_5BAD_5EED);

        //* When
        let lengths = 0..corpus.len();

        //* Then
        for len in lengths {
            assert_round_trips(&corpus[..len]);
            assert_round_trips(&vec![0x11u8; len]);
        }
    }

    #[test]
    fn decompress_with_a_truncated_buffer_fails() {
        //* Given
        // Too short to hold even the trailing length word.
        let data = [0u8; 2];

        //* When
        let result = decompress(&data, 8);

        //* Then
        assert!(matches!(
            result,
            Err(DecompressError::BufferTooSmall { .. })
        ));
    }

    #[test]
    fn decompress_with_a_trailer_claiming_more_than_the_buffer_fails() {
        //* Given
        // A non-zero extra length marks the stream packed, and the encoded length overruns.
        let mut data = vec![0u8; 16];
        data[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        data[8..12].copy_from_slice(&12u32.to_le_bytes());
        data[12..16].copy_from_slice(&1u32.to_le_bytes());

        //* When
        let result = decompress(&data, 8);

        //* Then
        assert!(matches!(
            result,
            Err(DecompressError::MalformedTrailer { .. })
        ));
    }

    #[test]
    fn decompress_with_a_header_size_below_the_trailer_fails() {
        //* Given
        let mut data = vec![0u8; 16];
        data[4..8].copy_from_slice(&16u32.to_le_bytes());
        data[8..12].copy_from_slice(&4u32.to_le_bytes());
        data[12..16].copy_from_slice(&1u32.to_le_bytes());

        //* When
        let result = decompress(&data, 8);

        //* Then
        assert!(matches!(
            result,
            Err(DecompressError::MalformedTrailer { .. })
        ));
    }

    #[test]
    fn longest_match_with_empty_input_matches_brute_force() {
        //* Given
        let data: Vec<u8> = Vec::new();
        let finder = MatchFinder::new(&data);

        //* When
        let matches = longest_match_at_every_position(&finder);

        //* Then
        assert_matches_brute_force(&data, &matches);
    }

    #[test]
    fn longest_match_with_one_byte_input_matches_brute_force() {
        //* Given
        // Fewer than MIN_MATCH bytes exercises the end-of-buffer guard.
        let data = [0x42u8];
        let finder = MatchFinder::new(&data);

        //* When
        let matches = longest_match_at_every_position(&finder);

        //* Then
        assert_matches_brute_force(&data, &matches);
    }

    #[test]
    fn longest_match_with_two_byte_input_matches_brute_force() {
        //* Given
        // Fewer than MIN_MATCH bytes exercises the end-of-buffer guard.
        let data = [0x42u8, 0x43];
        let finder = MatchFinder::new(&data);

        //* When
        let matches = longest_match_at_every_position(&finder);

        //* Then
        assert_matches_brute_force(&data, &matches);
    }

    #[test]
    fn longest_match_with_rle_input_matches_brute_force() {
        //* Given
        // A run longer than MAX_OFFSET exercises the overlap cap, the MAX_MATCH
        // early-exit, and the window boundary.
        let data = vec![0xABu8; MAX_OFFSET + 256];
        let finder = MatchFinder::new(&data);

        //* When
        let matches = longest_match_at_every_position(&finder);

        //* Then
        assert_matches_brute_force(&data, &matches);
    }

    #[test]
    fn longest_match_with_incompressible_input_matches_brute_force() {
        //* Given
        let data = pseudo_random_bytes(8192, 0x1234_5678_9ABC_DEF0);
        let finder = MatchFinder::new(&data);

        //* When
        let matches = longest_match_at_every_position(&finder);

        //* Then
        assert_matches_brute_force(&data, &matches);
    }

    #[test]
    fn longest_match_with_repeated_prefix_input_matches_brute_force() {
        //* Given
        // Many positions share the "11 22 33" prefix (long chains), while a
        // varying fourth byte keeps most matches at length three.
        let mut data = Vec::new();
        for i in 0..600u32 {
            data.extend_from_slice(&[0x11, 0x22, 0x33]);
            data.push((i & 0xFF) as u8);
        }
        let finder = MatchFinder::new(&data);

        //* When
        let matches = longest_match_at_every_position(&finder);

        //* Then
        assert_matches_brute_force(&data, &matches);
    }

    #[test]
    fn longest_match_at_max_offset_boundary_matches_brute_force() {
        //* Given
        // An identical pattern at the start and exactly MAX_OFFSET away, so the
        // matching candidate sits on the window edge.
        let pattern = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut data = pseudo_random_bytes(MAX_OFFSET + pattern.len() + 16, 0xDEAD_BEEF_CAFE_F00D);
        data[..pattern.len()].copy_from_slice(&pattern);
        data[MAX_OFFSET..MAX_OFFSET + pattern.len()].copy_from_slice(&pattern);
        let finder = MatchFinder::new(&data);

        //* When
        let matches = longest_match_at_every_position(&finder);

        //* Then
        assert_matches_brute_force(&data, &matches);
    }

    #[test]
    fn longest_match_with_structured_input_matches_brute_force() {
        //* Given
        // Text-like data with repeated substrings.
        let data = b"the quick brown fox jumps over the lazy dog. ".repeat(64);
        let finder = MatchFinder::new(&data);

        //* When
        let matches = longest_match_at_every_position(&finder);

        //* Then
        assert_matches_brute_force(&data, &matches);
    }
}
