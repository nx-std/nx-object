//! The single-level hash table that verifies a PFS0 section.
//!
//! Where a RomFS section carries a whole tree, a PFS0 section carries one table: a SHA-256 per block
//! of archive, and a master hash in the superblock covering the table. The table leads the section
//! and the archive follows it, which is why the superblock records where the archive starts rather
//! than assuming it.
//!
//! The table is padded up to the media unit so the archive begins on a boundary the section entry
//! can express. As in [`super::ivfc`], the padding is unconditional, so a table already on a
//! boundary still gains a whole unit; nothing reads the padding, and the superblock's recorded
//! archive offset is what locates the archive either way.

use alloc::{vec, vec::Vec};

use sha2::{Digest as _, Sha256};

use crate::raw::nca::{MEDIA_UNIT_SIZE, Pfs0Superblock};

/// Bytes in a SHA-256 digest.
const HASH_SIZE: usize = 0x20;

/// A built PFS0 section: the bytes to write and the superblock describing them.
pub struct PartitionSection {
    /// The padded hash table followed by the archive, ready to be written into the NCA.
    pub data: Vec<u8>,
    /// The superblock the section's FS header carries.
    pub superblock: Pfs0Superblock,
}

/// Build the section covering `archive`, hashing it in `block_size`-byte blocks.
pub fn build(archive: Vec<u8>, block_size: u32) -> PartitionSection {
    let mut hash_table = Vec::with_capacity((archive.len() / block_size as usize + 1) * HASH_SIZE);
    for block in archive.chunks(block_size as usize) {
        let digest: [u8; HASH_SIZE] = Sha256::digest(block).into();
        hash_table.extend_from_slice(&digest);
    }

    let hash_table_size = hash_table.len() as u64;
    let master_hash: [u8; HASH_SIZE] = Sha256::digest(&hash_table).into();

    // Unconditional, so a table already on a media boundary still gains a whole unit. Nothing reads
    // the padding; `pfs0_offset` records where it leaves the archive. The media unit is a 0x200
    // constant, so narrowing it to `usize` is exact on every target.
    let media_unit = MEDIA_UNIT_SIZE as usize;
    let padding = media_unit - (hash_table.len() % media_unit);
    hash_table.extend_from_slice(&vec![0u8; padding]);
    let archive_offset = hash_table.len() as u64;

    let superblock = Pfs0Superblock {
        master_hash,
        block_size: block_size.into(),
        always_2: 2.into(),
        hash_table_offset: 0.into(),
        hash_table_size: hash_table_size.into(),
        pfs0_offset: archive_offset.into(),
        pfs0_size: (archive.len() as u64).into(),
        _reserved: [0; 0xF0],
    };

    let mut data = hash_table;
    data.extend_from_slice(&archive);

    PartitionSection { data, superblock }
}

#[cfg(test)]
mod tests {
    use super::build;
    use crate::raw::nca::MEDIA_UNIT_SIZE;

    #[test]
    fn build_records_the_archive_after_the_padded_table() {
        //* Given
        let archive = vec![0x7Eu8; 0x800];

        //* When
        let section = build(archive.clone(), 0x1000);

        //* Then
        let archive_offset = section.superblock.pfs0_offset.get();
        assert_eq!(
            archive_offset % MEDIA_UNIT_SIZE,
            0,
            "the archive must start on a media boundary"
        );
        assert_eq!(
            &section.data[archive_offset as usize..],
            &archive[..],
            "the archive should follow the padded table unchanged"
        );
    }

    #[test]
    fn build_sizes_the_table_by_the_number_of_blocks() {
        //* Given
        // Three blocks of 0x1000, the last one short.
        let archive = vec![0u8; 0x2001];

        //* When
        let section = build(archive, 0x1000);

        //* Then
        assert_eq!(
            section.superblock.hash_table_size.get(),
            3 * 0x20,
            "one SHA-256 per block, short final block included"
        );
    }

    #[test]
    fn build_excludes_the_padding_from_the_recorded_table_size() {
        //* Given
        let archive = vec![0u8; 0x1000];

        //* When
        let section = build(archive, 0x1000);

        //* Then
        assert!(
            section.superblock.hash_table_size.get() < section.superblock.pfs0_offset.get(),
            "the recorded size covers the hashes, the offset covers them plus padding"
        );
    }
}
