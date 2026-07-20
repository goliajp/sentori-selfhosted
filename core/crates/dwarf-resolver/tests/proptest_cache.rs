//! Property tests for `ResolverCache` and `MachoSlicer`.
//!
//! The cache's invariants mirror what `sourcemap-resolver` pins
//! down (LRU bound, insert/get round-trip, idempotent remove,
//! eviction order). The slicer's invariants are around fat-Mach-O
//! roundtrips: an arbitrary list of (arch, payload) packed into a
//! fat header always slices back to the same payloads under
//! `MachoSlicer::slice`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    missing_docs
)]

use core::num::NonZeroUsize;
use std::sync::Arc;

use object::write::{Object, StandardSegment};
use object::{Architecture, BinaryFormat, Endianness, SectionKind};
use proptest::prelude::*;

use sentori_dwarf_resolver::{Arch, DwarfModule, MachoSlicer, ResolverCache, SliceError};

/// Build a tiny throw-away Mach-O whose only purpose is to be a
/// distinct payload — the cache + slicer don't care about DWARF;
/// they only see bytes.
fn tiny_macho(arch: Architecture, marker: u8) -> Vec<u8> {
    let mut obj = Object::new(BinaryFormat::MachO, arch, Endianness::Little);
    let seg = obj.segment_name(StandardSegment::Text).to_vec();
    let text = obj.add_section(seg, b"__text".to_vec(), SectionKind::Text);
    obj.append_section_data(text, &[marker; 64], 16);
    obj.write().expect("write")
}

/// Same trivial fat builder the unit tests use, exported here so
/// proptests don't reach into `crate::test_fixtures` (which is
/// `#[cfg(test)]` private).
fn build_fat32(slices: &[(Arch, Vec<u8>)]) -> Vec<u8> {
    let header_len = 8usize;
    let arch_len = 20usize;
    let n = u32::try_from(slices.len()).expect("count");
    let align_pow = 12u32;
    let align = 1u64 << align_pow;

    let mut payload_offsets = Vec::with_capacity(slices.len());
    let mut cursor = header_len as u64 + arch_len as u64 * u64::from(n);
    for (_, payload) in slices {
        cursor = cursor.div_ceil(align) * align;
        payload_offsets.push(cursor);
        cursor += payload.len() as u64;
    }

    let total = cursor as usize;
    let mut out = vec![0u8; total];
    out[..4].copy_from_slice(&0xCAFE_BABEu32.to_be_bytes());
    out[4..8].copy_from_slice(&n.to_be_bytes());

    for (i, ((arch, payload), &offset)) in slices.iter().zip(payload_offsets.iter()).enumerate() {
        let s = header_len + i * arch_len;
        out[s..s + 4].copy_from_slice(&arch.cputype().to_be_bytes());
        out[s + 4..s + 8].copy_from_slice(&arch.cpusubtype().to_be_bytes());
        out[s + 8..s + 12].copy_from_slice(&u32::try_from(offset).expect("fits").to_be_bytes());
        out[s + 12..s + 16]
            .copy_from_slice(&u32::try_from(payload.len()).expect("fits").to_be_bytes());
        out[s + 16..s + 20].copy_from_slice(&align_pow.to_be_bytes());

        let o = offset as usize;
        out[o..o + payload.len()].copy_from_slice(payload);
    }
    out
}

fn arc_module() -> Arc<DwarfModule> {
    // Reuse the same minimal DWARF the internal tests use; we don't
    // need full symbolication here, just any parsable Arc payload.
    // Easiest way: defer to the roundtrip integration test's
    // builder via copy. Since proptest files can't `mod`-import
    // from each other, inline a tiny no-symbolic Mach-O that at
    // least has DWARF placeholders to satisfy NoDwarfSections.
    let bytes = build_module_with_min_dwarf();
    Arc::new(DwarfModule::from_bytes(bytes).expect("parse"))
}

fn build_module_with_min_dwarf() -> Vec<u8> {
    use gimli::write::{EndianVec, Sections};
    use gimli::{Encoding, Format, LittleEndian, SectionId};

    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 4,
        address_size: 8,
    };
    let mut dwarf = gimli::write::Dwarf::new();
    let lp = gimli::write::LineProgram::new(
        encoding,
        gimli::LineEncoding::default(),
        gimli::write::LineString::String(b"/p".to_vec()),
        None,
        gimli::write::LineString::String(b"a.rs".to_vec()),
        None,
    );
    dwarf.units.add(gimli::write::Unit::new(encoding, lp));
    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections).expect("emit");

    let mut obj = Object::new(
        BinaryFormat::MachO,
        Architecture::Aarch64,
        Endianness::Little,
    );
    sections
        .for_each(|id: SectionId, data| -> Result<(), ()> {
            if data.slice().is_empty() {
                return Ok(());
            }
            let raw = id.name();
            let name = raw
                .strip_prefix(".debug_")
                .map_or_else(|| raw.to_owned(), |s| format!("__debug_{s}"));
            let seg = obj.segment_name(StandardSegment::Debug).to_vec();
            let sect = obj.add_section(seg, name.into_bytes(), SectionKind::Debug);
            obj.append_section_data(sect, data.slice(), 1);
            Ok(())
        })
        .expect("iter");

    obj.write().expect("write")
}

fn arch_strategy() -> impl Strategy<Value = Arch> {
    prop_oneof![
        Just(Arch::Arm64),
        Just(Arch::X86_64),
        Just(Arch::Arm64e),
        Just(Arch::Armv7),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        .. ProptestConfig::default()
    })]

    /// `len ≤ capacity` always — independent of insertion order.
    #[test]
    fn cache_len_never_exceeds_capacity(
        keys in prop::collection::vec(0u32..1000, 1..50),
        cap in 1usize..16,
    ) {
        let m = arc_module();
        let c: ResolverCache<u32> =
            ResolverCache::new(NonZeroUsize::new(cap).expect("non-zero"));
        for k in &keys {
            c.insert(*k, Arc::clone(&m));
        }
        prop_assert!(c.len() <= cap);
    }

    /// `insert(k, v)` + `get(k)` returns the same `Arc` (until evicted).
    #[test]
    fn cache_insert_then_get_roundtrips(key in 0u32..1000) {
        let m = arc_module();
        let c: ResolverCache<u32> =
            ResolverCache::new(NonZeroUsize::new(4).expect("non-zero"));
        c.insert(key, Arc::clone(&m));
        let got = c.get(&key).expect("just inserted");
        prop_assert!(Arc::ptr_eq(&got, &m));
    }

    /// Idempotent remove.
    #[test]
    fn cache_remove_is_idempotent(key in 0u32..1000) {
        let m = arc_module();
        let c: ResolverCache<u32> =
            ResolverCache::new(NonZeroUsize::new(4).expect("non-zero"));
        c.insert(key, m);
        prop_assert!(c.remove(&key).is_some());
        prop_assert!(c.remove(&key).is_none());
    }

    /// Slicer roundtrip: a fat built from N (arch, bytes) entries
    /// returns each entry's bytes verbatim under the matching arch.
    #[test]
    fn slicer_extracts_each_slice_verbatim(
        n in 1usize..4,
        marker in 0u8..=255,
    ) {
        // Build N distinct slices, each with a distinct arch + marker.
        let arches = [Arch::Arm64, Arch::X86_64, Arch::Arm64e, Arch::Armv7];
        let archis = [
            Architecture::Aarch64,
            Architecture::X86_64,
            Architecture::Aarch64,
            Architecture::Arm,
        ];
        let mut slices = Vec::new();
        for i in 0..n {
            slices.push((arches[i], tiny_macho(archis[i], marker.wrapping_add(i as u8))));
        }
        let fat = build_fat32(&slices);
        for (a, expected) in &slices {
            let got = MachoSlicer::slice(&fat, *a).expect("slice");
            prop_assert_eq!(&got, expected);
        }
    }

    /// Slicer reports the right "available" set on miss.
    #[test]
    fn slicer_reports_available_on_miss(
        marker in 0u8..=255,
    ) {
        let arm = tiny_macho(Architecture::Aarch64, marker);
        let fat = build_fat32(&[(Arch::Arm64, arm)]);
        let err = MachoSlicer::slice(&fat, Arch::X86_64).expect_err("no x86_64");
        match err {
            SliceError::ArchNotFound { requested, available } => {
                prop_assert_eq!(requested, Arch::X86_64);
                prop_assert_eq!(available, vec![Arch::Arm64]);
            }
            other => prop_assert!(false, "wrong: {other:?}"),
        }
    }

    /// `list_arches` matches the input ordering.
    #[test]
    fn list_arches_preserves_insertion_order(
        a in arch_strategy(),
        b in arch_strategy(),
    ) {
        prop_assume!(a != b); // distinct arches only
        let pa = tiny_macho(Architecture::Aarch64, 0xAA);
        let pb = tiny_macho(Architecture::X86_64, 0xBB);
        let fat = build_fat32(&[(a, pa), (b, pb)]);
        let listed = MachoSlicer::list_arches(&fat).expect("list");
        prop_assert_eq!(listed, vec![a, b]);
    }
}
