//! Fat (universal) Mach-O slicer.
//!
//! iOS / macOS dSYM bundles are typically built as fat binaries
//! carrying one slice per shipped architecture (`arm64`, `arm64e`,
//! `x86_64`). The DWARF resolver only ever wants one slice at a
//! time — picking it out is fiddly enough (two fat-header layouts,
//! 32-bit and 64-bit offsets, big-endian header, masked subtype
//! bits) that we encapsulate it here so callers don't pull `object`
//! into their public surface just to find the right slab of bytes.
//!
//! The slicer also handles the degenerate cases:
//!
//! - **Single-arch Mach-O** — returns [`SliceError::NotFat`] so the
//!   caller can branch to `DwarfModule::from_bytes` directly
//!   without doing its own header magic check.
//! - **Fat without the requested arch** — returns
//!   [`SliceError::ArchNotFound`] with the arches actually present
//!   so the caller can surface a useful operator-facing hint
//!   ("dSYM upload missing arm64e slice").

use object::macho::{FAT_MAGIC, FAT_MAGIC_64, MH_CIGAM, MH_CIGAM_64, MH_MAGIC, MH_MAGIC_64};
use object::{Object as _, read::File};

use crate::arch::Arch;
use crate::error::{SliceError, SliceResult};

/// Stateless helper namespace for fat-Mach-O slicing.
pub struct MachoSlicer;

impl MachoSlicer {
    /// Slice the requested arch out of a fat (universal) Mach-O.
    ///
    /// Returns the bytes of the matching slice, copied out of the
    /// fat container as a fresh `Vec<u8>` ready to hand to
    /// [`crate::DwarfModule::from_bytes`].
    ///
    /// # Errors
    ///
    /// - [`SliceError::InvalidObject`] — the input is fat-shaped
    ///   but its arch table is malformed.
    /// - [`SliceError::TooShort`] — input is shorter than the fat
    ///   header.
    /// - [`SliceError::UnrecognisedMagic`] — the leading 4 bytes
    ///   match no known Mach-O magic.
    /// - [`SliceError::NotFat`] — the input is a valid single-arch
    ///   Mach-O. Use [`crate::DwarfModule::from_bytes`] directly.
    /// - [`SliceError::ArchNotFound`] — the fat carries no matching
    ///   slice. The error payload reports which arches were
    ///   present.
    pub fn slice(bytes: &[u8], requested: Arch) -> SliceResult<Vec<u8>> {
        match read_magic(bytes)? {
            Magic::Fat32 => slice_fat32(bytes, Some(requested)),
            Magic::Fat64 => slice_fat64(bytes, Some(requested)),
            Magic::Single => Err(SliceError::NotFat),
        }
    }

    /// List every arch present in a fat Mach-O.
    ///
    /// # Errors
    ///
    /// Same as [`Self::slice`] minus [`SliceError::ArchNotFound`]
    /// (the operation never asks for a specific arch).
    pub fn list_arches(bytes: &[u8]) -> SliceResult<Vec<Arch>> {
        match read_magic(bytes)? {
            Magic::Fat32 => list_fat32(bytes),
            Magic::Fat64 => list_fat64(bytes),
            Magic::Single => Err(SliceError::NotFat),
        }
    }

    /// Every slice's debug id (`LC_UUID`), uppercase hex without
    /// dashes — the identity a crashing frame carries as its
    /// `imageUuid`, and therefore the only thing that says whether a
    /// stored dSYM can serve a given stack.
    ///
    /// Until this existed the answer came from the uploaded
    /// *filename*. `sentori-cli` builds one that embeds the id, so
    /// the CLI path worked; a file uploaded by hand did not, and
    /// nothing anywhere could tell — the bytes parse, the artifact
    /// lists as usable, and no frame ever resolves against it.
    ///
    /// Load commands only. `object` parses lazily over the borrowed
    /// buffer and the fat slices are borrowed, not copied, so a
    /// 300 MB universal dSYM costs no allocation beyond the ids.
    ///
    /// Returns empty rather than erroring: a file with no `LC_UUID`
    /// is a fact about the file, not a failure to read it, and the
    /// caller's next move is the same either way.
    #[must_use]
    pub fn debug_ids(bytes: &[u8]) -> Vec<String> {
        let entries = match read_magic(bytes) {
            Ok(Magic::Fat32) => iter_fat32(bytes).unwrap_or_default(),
            Ok(Magic::Fat64) => iter_fat64(bytes).unwrap_or_default(),
            Ok(Magic::Single) => return uuid_of(bytes).into_iter().collect(),
            Err(_) => return Vec::new(),
        };
        let mut out: Vec<String> = Vec::new();
        for e in entries {
            let (Ok(off), Ok(sz)) = (usize::try_from(e.offset), usize::try_from(e.size)) else {
                continue;
            };
            let Some(end) = off.checked_add(sz) else {
                continue;
            };
            let Some(slice) = bytes.get(off..end) else {
                continue;
            };
            // A universal binary's slices are separate builds of the
            // same source and carry different ids. Duplicates would
            // still mean one file, so keep the set.
            if let Some(id) = uuid_of(slice)
                && !out.contains(&id)
            {
                out.push(id);
            }
        }
        out
    }
}

/// `LC_UUID` of a single (thin) Mach-O.
fn uuid_of(slice: &[u8]) -> Option<String> {
    let parsed = File::parse(slice).ok()?;
    let raw = parsed.mach_uuid().ok().flatten()?;
    let mut hex = String::with_capacity(32);
    for byte in raw {
        for nibble in [byte >> 4, byte & 0x0f] {
            let digit = char::from_digit(u32::from(nibble), 16)?;
            hex.push(digit.to_ascii_uppercase());
        }
    }
    Some(hex)
}

#[derive(Debug, Clone, Copy)]
enum Magic {
    Fat32,
    Fat64,
    Single,
}

fn read_magic(bytes: &[u8]) -> SliceResult<Magic> {
    if bytes.len() < 8 {
        return Err(SliceError::TooShort);
    }
    let first4: [u8; 4] = bytes[..4].try_into().map_err(|_| SliceError::TooShort)?;
    let big_endian_word = u32::from_be_bytes(first4);
    if big_endian_word == FAT_MAGIC {
        return Ok(Magic::Fat32);
    }
    if big_endian_word == FAT_MAGIC_64 {
        return Ok(Magic::Fat64);
    }
    let little_endian_word = u32::from_le_bytes(first4);
    match little_endian_word {
        MH_MAGIC | MH_MAGIC_64 | MH_CIGAM | MH_CIGAM_64 => Ok(Magic::Single),
        _ => Err(SliceError::UnrecognisedMagic(first4)),
    }
}

fn read_be_u32(bytes: &[u8], off: usize) -> SliceResult<u32> {
    let end = off.checked_add(4).ok_or(SliceError::TooShort)?;
    let slice = bytes.get(off..end).ok_or(SliceError::TooShort)?;
    let arr: [u8; 4] = slice.try_into().map_err(|_| SliceError::TooShort)?;
    Ok(u32::from_be_bytes(arr))
}

fn read_be_u64(bytes: &[u8], off: usize) -> SliceResult<u64> {
    let end = off.checked_add(8).ok_or(SliceError::TooShort)?;
    let slice = bytes.get(off..end).ok_or(SliceError::TooShort)?;
    let arr: [u8; 8] = slice.try_into().map_err(|_| SliceError::TooShort)?;
    Ok(u64::from_be_bytes(arr))
}

fn nfat(bytes: &[u8]) -> SliceResult<u32> {
    read_be_u32(bytes, 4)
}

/// 32-bit fat: header (8 bytes) followed by N×20-byte entries.
/// Each entry = `(cputype, cpusubtype, offset, size, align)` as
/// big-endian u32 fields.
fn iter_fat32(bytes: &[u8]) -> SliceResult<Vec<FatEntry>> {
    let n = nfat(bytes)? as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let base = 8 + i * 20;
        out.push(FatEntry {
            arch: Arch::from_raw(read_be_u32(bytes, base)?, read_be_u32(bytes, base + 4)?),
            offset: u64::from(read_be_u32(bytes, base + 8)?),
            size: u64::from(read_be_u32(bytes, base + 12)?),
        });
    }
    Ok(out)
}

/// 64-bit fat: header (8 bytes) followed by N×32-byte entries.
/// Each entry = `(cputype, cpusubtype, offset, size, align,
/// reserved)` where offset and size are 64-bit, the others 32-bit.
fn iter_fat64(bytes: &[u8]) -> SliceResult<Vec<FatEntry>> {
    let n = nfat(bytes)? as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let base = 8 + i * 32;
        out.push(FatEntry {
            arch: Arch::from_raw(read_be_u32(bytes, base)?, read_be_u32(bytes, base + 4)?),
            offset: read_be_u64(bytes, base + 8)?,
            size: read_be_u64(bytes, base + 16)?,
        });
    }
    Ok(out)
}

struct FatEntry {
    arch: Arch,
    offset: u64,
    size: u64,
}

fn slice_fat32(bytes: &[u8], requested: Option<Arch>) -> SliceResult<Vec<u8>> {
    slice_common(bytes, iter_fat32(bytes)?, requested)
}

fn slice_fat64(bytes: &[u8], requested: Option<Arch>) -> SliceResult<Vec<u8>> {
    slice_common(bytes, iter_fat64(bytes)?, requested)
}

fn list_fat32(bytes: &[u8]) -> SliceResult<Vec<Arch>> {
    Ok(iter_fat32(bytes)?.into_iter().map(|e| e.arch).collect())
}

fn list_fat64(bytes: &[u8]) -> SliceResult<Vec<Arch>> {
    Ok(iter_fat64(bytes)?.into_iter().map(|e| e.arch).collect())
}

fn slice_common(
    bytes: &[u8],
    entries: Vec<FatEntry>,
    requested: Option<Arch>,
) -> SliceResult<Vec<u8>> {
    let Some(want) = requested else {
        // Shouldn't reach — slicer always requests; defensive.
        return Err(SliceError::NotFat);
    };
    let mut available = Vec::with_capacity(entries.len());
    for e in entries {
        if e.arch == want {
            let off = usize::try_from(e.offset).map_err(|_| SliceError::TooShort)?;
            let sz = usize::try_from(e.size).map_err(|_| SliceError::TooShort)?;
            let end = off.checked_add(sz).ok_or(SliceError::TooShort)?;
            let payload = bytes.get(off..end).ok_or(SliceError::TooShort)?;
            return Ok(payload.to_vec());
        }
        available.push(e.arch);
    }
    Err(SliceError::ArchNotFound {
        requested: want,
        available,
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc
)]
mod tests {
    use super::*;
    use crate::test_fixtures::{build_fat32, synthetic_macho_with_dwarf, synthetic_macho_x86_64};

    /// A thin arm64 Mach-O carrying nothing but a header and the
    /// `LC_UUID` load command. Written by hand rather than by
    /// `object::write`, which emits no `LC_UUID` at all — so the
    /// encoder here and the decoder under test are two different
    /// pieces of code, and a shared misreading cannot pass.
    fn thin_with_uuid(uuid: [u8; 16]) -> Vec<u8> {
        const LC_UUID: u32 = 0x1b;
        let mut out = Vec::with_capacity(56);
        out.extend_from_slice(&MH_MAGIC_64.to_le_bytes());
        out.extend_from_slice(&0x0100_000cu32.to_le_bytes()); // CPU_TYPE_ARM64
        out.extend_from_slice(&0u32.to_le_bytes()); // cpusubtype
        out.extend_from_slice(&10u32.to_le_bytes()); // MH_DSYM
        out.extend_from_slice(&1u32.to_le_bytes()); // ncmds
        out.extend_from_slice(&24u32.to_le_bytes()); // sizeofcmds
        out.extend_from_slice(&0u32.to_le_bytes()); // flags
        out.extend_from_slice(&0u32.to_le_bytes()); // reserved
        out.extend_from_slice(&LC_UUID.to_le_bytes());
        out.extend_from_slice(&24u32.to_le_bytes()); // cmdsize
        out.extend_from_slice(&uuid);
        out
    }

    #[test]
    fn thin_macho_reports_its_debug_id() {
        let bytes = thin_with_uuid([
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54,
            0x32, 0x10,
        ]);
        assert_eq!(
            MachoSlicer::debug_ids(&bytes),
            vec!["0123456789ABCDEFFEDCBA9876543210".to_owned()]
        );
    }

    #[test]
    fn fat_reports_one_debug_id_per_slice() {
        let a = thin_with_uuid([0xaa; 16]);
        let b = thin_with_uuid([0xbb; 16]);
        let fat = build_fat32(&[(Arch::Arm64, &a), (Arch::X86_64, &b)]);
        let ids = MachoSlicer::debug_ids(&fat);
        assert_eq!(ids.len(), 2, "one id per slice, got {ids:?}");
        assert!(ids.contains(&"AA".repeat(16)));
        assert!(ids.contains(&"BB".repeat(16)));
    }

    /// The witness that this is reading the file rather than
    /// inventing an answer: `object::write` emits no `LC_UUID`, so a
    /// fixture built by it must come back with nothing.
    #[test]
    fn a_macho_without_lc_uuid_reports_nothing() {
        let f = synthetic_macho_with_dwarf();
        assert!(
            MachoSlicer::debug_ids(&f.bytes).is_empty(),
            "invented a debug id for a Mach-O that carries none"
        );
    }

    #[test]
    fn junk_reports_nothing_rather_than_panicking() {
        assert!(MachoSlicer::debug_ids(b"not a mach-o at all").is_empty());
        assert!(MachoSlicer::debug_ids(&[]).is_empty());
    }

    /// A real linker's output, not ours. Only where one is to hand.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_real_system_binary_reports_one_debug_id() {
        let Ok(bytes) = std::fs::read("/bin/ls") else {
            return;
        };
        let ids = MachoSlicer::debug_ids(&bytes);
        assert!(!ids.is_empty(), "/bin/ls carries an LC_UUID; we read none");
        for id in &ids {
            assert_eq!(id.len(), 32, "not a 16-byte uuid: {id}");
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_lowercase())
            );
        }
    }

    #[test]
    fn rejects_too_short_input() {
        let err = MachoSlicer::slice(&[1, 2], Arch::Arm64).expect_err("short");
        assert!(matches!(err, SliceError::TooShort));
    }

    #[test]
    fn rejects_unrecognised_magic() {
        let err = MachoSlicer::slice(&[0; 64], Arch::Arm64).expect_err("zeros");
        assert!(matches!(err, SliceError::UnrecognisedMagic(_)));
    }

    #[test]
    fn single_arch_returns_not_fat() {
        let fx = synthetic_macho_with_dwarf();
        let err = MachoSlicer::slice(&fx.bytes, Arch::Arm64).expect_err("single-arch");
        assert!(matches!(err, SliceError::NotFat));
    }

    #[test]
    fn fat_slicing_arm64() {
        let arm = synthetic_macho_with_dwarf();
        let x86 = synthetic_macho_x86_64();
        let fat = build_fat32(&[(Arch::Arm64, &arm.bytes), (Arch::X86_64, &x86.bytes)]);
        let extracted = MachoSlicer::slice(&fat, Arch::Arm64).expect("extract arm64");
        assert_eq!(extracted, arm.bytes);
    }

    #[test]
    fn fat_slicing_x86_64() {
        let arm = synthetic_macho_with_dwarf();
        let x86 = synthetic_macho_x86_64();
        let fat = build_fat32(&[(Arch::Arm64, &arm.bytes), (Arch::X86_64, &x86.bytes)]);
        let extracted = MachoSlicer::slice(&fat, Arch::X86_64).expect("extract x86_64");
        assert_eq!(extracted, x86.bytes);
    }

    #[test]
    fn fat_arch_not_found_reports_available() {
        let arm = synthetic_macho_with_dwarf();
        let fat = build_fat32(&[(Arch::Arm64, &arm.bytes)]);
        let err = MachoSlicer::slice(&fat, Arch::X86_64).expect_err("no x86_64");
        match err {
            SliceError::ArchNotFound {
                requested,
                available,
            } => {
                assert_eq!(requested, Arch::X86_64);
                assert_eq!(available, vec![Arch::Arm64]);
            }
            other => panic!("wrong error: {other:?}"),
        }
    }

    #[test]
    fn list_arches_returns_every_slice() {
        let arm = synthetic_macho_with_dwarf();
        let x86 = synthetic_macho_x86_64();
        let fat = build_fat32(&[(Arch::Arm64, &arm.bytes), (Arch::X86_64, &x86.bytes)]);
        let arches = MachoSlicer::list_arches(&fat).expect("list");
        assert_eq!(arches, vec![Arch::Arm64, Arch::X86_64]);
    }

    #[test]
    fn list_arches_on_single_returns_not_fat() {
        let fx = synthetic_macho_with_dwarf();
        let err = MachoSlicer::list_arches(&fx.bytes).expect_err("single");
        assert!(matches!(err, SliceError::NotFat));
    }
}
