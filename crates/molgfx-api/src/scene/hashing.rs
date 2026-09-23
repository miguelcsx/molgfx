//! The content digest of one molecular source.
//!
//! A digest is published in the specification as `content_hash`, exported
//! through `MolViewSpec` and verified when a specification is reimported, so its
//! value is a contract: a change to it invalidates every saved scene. The bytes
//! fed to the digest are therefore part of the interface, and this module is
//! where they are decided.

use sha2::{Digest, Sha256};

/// Bytes hashed per `update` call when a column is fed to the digest.
///
/// The digest sees the same byte stream whatever the block size, so this only
/// trades calls against a stack buffer: large enough that the calls are rare,
/// small enough to stay off the heap entirely.
const HASH_BLOCK_BYTES: usize = 4096;

/// Content digest of one molecular source: coordinates, topology and the
/// parser's own labels.
///
/// Published in the specification as `content_hash`, exported through
/// `MolViewSpec` and verified when a specification is reimported, so the value
/// a contract: a change to it invalidates every saved scene.
#[must_use]
pub fn structure_hash(structure: &molgfx_core::MolecularSource) -> Box<str> {
    let mut hash = Sha256::new();
    // One `update` per logical group rather than one per 4-byte field. The
    // digest is identical either way — chunking is not part of SHA-256's output
    // — but a per-lane `update` costs a call and a buffered copy per field,
    // which dominated scene construction on large structures.
    // A fixed 64-bit width, not `usize`. This digest crosses a process and an
    // architecture boundary — a Python binding on a 64-bit host publishes the
    // descriptor that a wasm32 consumer verifies — and `usize::to_le_bytes`
    // writes eight bytes on one side and four on the other, so the two could
    // never agree. Every field in this digest is width-explicit for that reason.
    hash.update(hashed_length(structure.coordinates().len()));
    hash_coordinate_lanes(&mut hash, structure.coordinates());
    hash_atoms(&mut hash, &structure.topology().atoms);
    hash_bonds(&mut hash, &structure.topology().bonds);
    if let Some(native) = structure.molframe() {
        hash_native_text(&mut hash, native);
    }
    format!("{digest:x}", digest = hash.finalize()).into_boxed_str()
}

/// Feeds every coordinate lane as one contiguous run.
///
/// The lanes are hashed in bounded blocks rather than through a buffer of the
/// whole column: the digest covers the same bytes in the same order, and the
/// transient copy of the coordinate array — the largest allocation a scene
/// construction makes — disappears.
fn hash_coordinate_lanes(hash: &mut Sha256, coordinates: &[[f32; 3]]) {
    let mut block = [0_u8; HASH_BLOCK_BYTES];
    let mut filled = 0;
    for coordinate in coordinates {
        for lane in coordinate {
            let bytes = lane.to_bits().to_le_bytes();
            if filled + bytes.len() > block.len() {
                hash.update(&block[..filled]);
                filled = 0;
            }
            block[filled..filled + bytes.len()].copy_from_slice(&bytes);
            filled += bytes.len();
        }
    }
    hash.update(&block[..filled]);
}

/// Feeds every atom's element and residue as one run.
fn hash_atoms(hash: &mut Sha256, atoms: &[molgfx_core::SourceAtom]) {
    let mut block = [0_u8; HASH_BLOCK_BYTES];
    let mut filled = 0;
    for atom in atoms {
        let mut entry = [0_u8; 5];
        entry[0] = atom.element;
        entry[1..].copy_from_slice(&atom.residue.to_le_bytes());
        if filled + entry.len() > block.len() {
            hash.update(&block[..filled]);
            filled = 0;
        }
        block[filled..filled + entry.len()].copy_from_slice(&entry);
        filled += entry.len();
    }
    hash.update(&block[..filled]);
}

/// Feeds every bond's endpoints and aromaticity as one run.
fn hash_bonds(hash: &mut Sha256, bonds: &[molgfx_core::SourceBond]) {
    let mut block = [0_u8; HASH_BLOCK_BYTES];
    let mut filled = 0;
    for bond in bonds {
        let mut entry = [0_u8; 9];
        entry[..4].copy_from_slice(&bond.atoms[0].to_le_bytes());
        entry[4..8].copy_from_slice(&bond.atoms[1].to_le_bytes());
        entry[8] = u8::from(bond.aromatic);
        if filled + entry.len() > block.len() {
            hash.update(&block[..filled]);
            filled = 0;
        }
        block[filled..filled + entry.len()].copy_from_slice(&entry);
        filled += entry.len();
    }
    hash.update(&block[..filled]);
}

/// Feeds the parser's own labels, keeping the per-field length prefix that
/// distinguishes an absent label from an empty one.
fn hash_native_text(hash: &mut Sha256, native: &molframe::Structure) {
    for chain in native.chains() {
        hash_text(hash, chain.label());
        hash_text(hash, chain.auth_label());
        for residue in chain.residues() {
            hash_text(hash, residue.name());
            hash_text(hash, residue.auth_name());
            hash_text(hash, residue.ins_code());
            hash_seq_id(hash, residue.label_seq_id());
            hash_seq_id(hash, residue.auth_seq_id());
            hash.update([u8::from(residue.is_het())]);
            for atom in residue.atoms() {
                hash_text(hash, atom.name());
                hash_text(hash, atom.auth_name());
                hash_text(hash, atom.alt_label());
            }
        }
    }
}

/// Feeds one sequence identifier, or `i32::MIN` when absent.
fn hash_seq_id(hash: &mut Sha256, value: Option<i32>) {
    hash.update(
        value
            .into_iter()
            .fold(i32::MIN, |_, value| value)
            .to_le_bytes(),
    );
}

fn hash_text(hash: &mut Sha256, value: Option<&str>) {
    let value = value.map_or(&[][..], str::as_bytes);
    // Width-explicit for the same reason as the coordinate count: a label
    // length written as `usize` differs between the publishing host and a
    // wasm32 consumer.
    hash.update(hashed_length(value.len()));
    hash.update(value);
}

/// One length as the eight little-endian bytes this digest is defined over.
///
/// The digest crosses a process and an architecture boundary, so a length that
/// encoded itself at the platform's pointer width would differ between the
/// 64-bit host that publishes a scene and the wasm32 consumer that verifies it:
/// `usize::to_le_bytes` writes eight bytes on one side and four on the other.
/// The conversion is written out rather than folded through a default, because
/// the caller decides what an unrepresentable length means here.
fn hashed_length(length: usize) -> [u8; 8] {
    match u64::try_from(length) {
        Ok(width) => width.to_le_bytes(),
        // No allocation reaches this digest on a target whose address space is
        // narrower than `u64`, so this is unreachable in practice; it is a
        // distinct value rather than a silent truncation, which would make two
        // different structures share one digest.
        Err(_) => u64::MAX.to_le_bytes(),
    }
}
