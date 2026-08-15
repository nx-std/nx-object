//! The IVFC hash tree that verifies a RomFS section.
//!
//! The tree is built from the top down: the RomFS image is the last level, the level below it holds
//! one SHA-256 per `0x4000` bytes of it, and so on for six levels in total. The master hash in the
//! header then covers level 0 alone, so verifying any block of the image means hashing one block per
//! level rather than the whole image.
//!
//! Every level except the last is padded up to a whole number of blocks, because the level below
//! hashes it in fixed-size chunks and a short final chunk would hash different bytes on the console
//! than it did here. The padding is unconditional, so a level already a multiple of the block size
//! still gains a full empty block; its `hash_data_size` counts that block, and the level below hashes
//! it like any other, so the tree stays consistent either way.

use alloc::{vec, vec::Vec};

use sha2::{Digest as _, Sha256};

use crate::raw::nca::{IVFC_MAX_LEVELS, IvfcLevelHeader};

/// Bytes of the level above that one hash in a level covers.
pub const IVFC_BLOCK_SIZE: u64 = 0x4000;

/// Base-2 logarithm of [`IVFC_BLOCK_SIZE`], which is how the level header stores it.
const IVFC_BLOCK_SIZE_LOG2: u32 = 14;

/// Bytes in a SHA-256 digest.
const HASH_SIZE: usize = 0x20;

/// A built hash tree: the levels to write, the headers describing them, and the root hash.
pub struct IvfcTree {
    /// Levels 0 through 5 concatenated, in the order they are written into the section.
    pub data: Vec<u8>,
    /// One header per level, low to high, with logical offsets already accumulated.
    pub level_headers: [IvfcLevelHeader; IVFC_MAX_LEVELS],
    /// SHA-256 of level 0 in full, padding included.
    pub master_hash: [u8; HASH_SIZE],
}

/// Build the hash tree covering `romfs`, which becomes its last level.
pub fn build(romfs: Vec<u8>) -> IvfcTree {
    // Levels are derived downwards from the image, so they are produced in reverse order and
    // reversed once the whole tree exists. The previous level is carried in a binding rather than
    // read back out of the vector, which leaves no absent case to discharge.
    let mut levels: Vec<Vec<u8>> = Vec::with_capacity(IVFC_MAX_LEVELS);
    let mut level_above = romfs;
    while levels.len() < IVFC_MAX_LEVELS - 1 {
        let level_below = hash_level(&level_above);
        levels.push(level_above);
        level_above = level_below;
    }
    levels.push(level_above);

    levels.reverse();

    let master_hash: [u8; HASH_SIZE] = Sha256::digest(&levels[0]).into();

    let mut level_headers = [IvfcLevelHeader::default(); IVFC_MAX_LEVELS];
    let mut logical_offset = 0u64;
    for (header, level) in level_headers.iter_mut().zip(levels.iter()) {
        header.logical_offset = logical_offset.into();
        header.hash_data_size = (level.len() as u64).into();
        header.block_size = IVFC_BLOCK_SIZE_LOG2.into();
        logical_offset += level.len() as u64;
    }

    let mut data = Vec::with_capacity(logical_offset as usize);
    for level in levels {
        data.extend_from_slice(&level);
    }

    IvfcTree {
        data,
        level_headers,
        master_hash,
    }
}

/// Hash `level` block by block and pad the result out to a whole number of blocks.
fn hash_level(level: &[u8]) -> Vec<u8> {
    // The block size is a 0x4000 constant, so narrowing it to `usize` is exact on every target.
    let block_size = IVFC_BLOCK_SIZE as usize;
    let mut hashes = Vec::with_capacity((level.len() / block_size + 1) * HASH_SIZE);

    for block in level.chunks(block_size) {
        let digest: [u8; HASH_SIZE] = Sha256::digest(block).into();
        hashes.extend_from_slice(&digest);
    }

    // Unconditional, so a level already a whole number of blocks gains an empty one. The recorded
    // size counts it and the level below hashes it, so the extra block costs space and nothing else.
    let padding = block_size - (hashes.len() % block_size);
    hashes.extend_from_slice(&vec![0u8; padding]);

    hashes
}

#[cfg(test)]
mod tests {
    use super::{IVFC_BLOCK_SIZE, build};
    use crate::raw::nca::IVFC_MAX_LEVELS;

    #[test]
    fn build_makes_the_image_the_last_level() {
        //* Given
        let romfs = vec![0x5Au8; 100];

        //* When
        let tree = build(romfs.clone());

        //* Then
        let last = &tree.level_headers[IVFC_MAX_LEVELS - 1];
        assert_eq!(
            last.hash_data_size.get(),
            romfs.len() as u64,
            "the image is stored as-is and is not padded"
        );
        assert_eq!(
            &tree.data[tree.data.len() - romfs.len()..],
            &romfs[..],
            "the image should close the concatenated levels"
        );
    }

    #[test]
    fn build_pads_every_derived_level_to_a_whole_block() {
        //* Given
        let romfs = vec![0u8; 1];

        //* When
        let tree = build(romfs);

        //* Then
        for (index, header) in tree.level_headers[..IVFC_MAX_LEVELS - 1].iter().enumerate() {
            assert_eq!(
                header.hash_data_size.get() % IVFC_BLOCK_SIZE,
                0,
                "level {index} should be a whole number of blocks"
            );
        }
    }

    #[test]
    fn build_accumulates_logical_offsets_across_the_levels() {
        //* Given
        let romfs = vec![0u8; 0x10000];

        //* When
        let tree = build(romfs);

        //* Then
        assert_eq!(
            tree.level_headers[0].logical_offset.get(),
            0,
            "the first level starts the logical space"
        );
        for window in tree.level_headers.windows(2) {
            let (lower, upper) = (&window[0], &window[1]);
            assert_eq!(
                upper.logical_offset.get(),
                lower.logical_offset.get() + lower.hash_data_size.get(),
                "each level should start where the one below it ended"
            );
        }
    }

    #[test]
    fn build_is_deterministic_for_the_same_image() {
        //* Given
        let romfs = vec![0x11u8; 0x4321];

        //* When
        let first = build(romfs.clone());
        let second = build(romfs);

        //* Then
        assert_eq!(
            first.master_hash, second.master_hash,
            "the same image should produce the same root hash"
        );
        assert_eq!(first.data, second.data);
    }
}
