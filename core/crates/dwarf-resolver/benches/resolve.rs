//! Criterion benches for the dwarf-resolver stone.
//!
//! Three hot paths cover realistic usage:
//!
//! 1. `parse_minimal_dwarf` — cold Mach-O+DWARF bytes →
//!    [`DwarfModule`]. Establishes the cost a cache miss pays.
//!    Real iOS dSYMs are 5-50 MB; this synthetic fixture is much
//!    smaller, but the parse cost dominates either way (linear in
//!    the .debug_info size).
//! 2. `resolve_known_offset` — repeated lookups against a parsed
//!    module. Establishes per-frame cost for a typical 20-frame
//!    stack symbolication once the module is hot.
//! 3. `cache_get_all_hit` — `ResolverCache::get` steady-state hit
//!    rate. Establishes the per-frame cache overhead under burst.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::doc_markdown,
    missing_docs
)]

use core::num::NonZeroUsize;
use std::hint::black_box;
use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use gimli::write::{Address, AttributeValue, EndianVec, LineProgram, LineString, Sections};
use gimli::{DW_AT_comp_dir, DW_AT_decl_file, DW_AT_decl_line, DW_AT_high_pc, DW_AT_language};
use gimli::{DW_AT_low_pc, DW_AT_name, DW_LANG_Rust, DW_TAG_subprogram};
use gimli::{Encoding, Format, LineEncoding, LittleEndian, SectionId};
use object::write::{Object, StandardSegment};
use object::{Architecture, BinaryFormat, Endianness, SectionKind};

use sentori_dwarf_resolver::{DwarfModule, ResolverCache};

const LOW_PC: u64 = 0x1000;
const FN_LEN: u64 = 0x400;

fn build_bytes(n_subprograms: u32) -> Vec<u8> {
    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 4,
        address_size: 8,
    };
    let mut dwarf = gimli::write::Dwarf::new();
    let lp = LineProgram::new(
        encoding,
        LineEncoding::default(),
        LineString::String(b"/build".to_vec()),
        None,
        LineString::String(b"src/main.rs".to_vec()),
        None,
    );
    let mut unit = gimli::write::Unit::new(encoding, lp);

    let dir = unit
        .line_program
        .add_directory(LineString::String(b"/build".to_vec()));
    let file_id =
        unit.line_program
            .add_file(LineString::String(b"src/main.rs".to_vec()), dir, None);

    let cd = dwarf.strings.add("/build");
    let nm = dwarf.strings.add("src/main.rs");

    let root = unit.root();
    let r = unit.get_mut(root);
    r.set(DW_AT_comp_dir, AttributeValue::StringRef(cd));
    r.set(DW_AT_name, AttributeValue::StringRef(nm));
    r.set(DW_AT_language, AttributeValue::Language(DW_LANG_Rust));
    r.set(
        DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(LOW_PC)),
    );
    r.set(
        DW_AT_high_pc,
        AttributeValue::Udata(u64::from(n_subprograms) * FN_LEN),
    );

    for i in 0..n_subprograms {
        let fname = dwarf.strings.add(format!("fn_{i}"));
        let sub = unit.add(root, DW_TAG_subprogram);
        let s = unit.get_mut(sub);
        s.set(DW_AT_name, AttributeValue::StringRef(fname));
        s.set(
            DW_AT_low_pc,
            AttributeValue::Address(Address::Constant(LOW_PC + u64::from(i) * FN_LEN)),
        );
        s.set(DW_AT_high_pc, AttributeValue::Udata(FN_LEN));
        s.set(DW_AT_decl_file, AttributeValue::FileIndex(Some(file_id)));
        s.set(DW_AT_decl_line, AttributeValue::Udata(u64::from(i + 1)));
    }

    let lp = &mut unit.line_program;
    for i in 0..n_subprograms {
        lp.begin_sequence(Some(Address::Constant(LOW_PC + u64::from(i) * FN_LEN)));
        lp.row().file = file_id;
        lp.row().line = u64::from(i + 1);
        lp.row().address_offset = 0;
        lp.generate_row();
        lp.end_sequence(FN_LEN);
    }
    dwarf.units.add(unit);

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

    let total = (LOW_PC + u64::from(n_subprograms) * FN_LEN) as usize;
    let text_seg = obj.segment_name(StandardSegment::Text).to_vec();
    let text = obj.add_section(text_seg, b"__text".to_vec(), SectionKind::Text);
    obj.append_section_data(text, &vec![0u8; total], 16);

    obj.write().expect("write")
}

fn bench_parse(c: &mut Criterion) {
    let small = build_bytes(1);
    let medium = build_bytes(64);
    c.bench_function("parse_minimal_dwarf_1_fn", |b| {
        b.iter(|| {
            let m = DwarfModule::from_bytes(black_box(small.clone())).expect("parse small");
            black_box(m);
        });
    });
    c.bench_function("parse_dwarf_64_fns", |b| {
        b.iter(|| {
            let m = DwarfModule::from_bytes(black_box(medium.clone())).expect("parse medium");
            black_box(m);
        });
    });
}

fn bench_resolve(c: &mut Criterion) {
    let bytes = build_bytes(64);
    let module = DwarfModule::from_bytes(bytes).expect("parse for resolve");
    c.bench_function("resolve_known_offset", |b| {
        let mut i: u64 = 0;
        b.iter(|| {
            // Cycle through all 64 functions to defeat constant-folding.
            let off = LOW_PC + (i % 64) * FN_LEN;
            i = i.wrapping_add(1);
            let r = module.resolve(black_box(off)).expect("resolve");
            black_box(r);
        });
    });
}

fn bench_cache(c: &mut Criterion) {
    let bytes = build_bytes(8);
    let module = Arc::new(DwarfModule::from_bytes(bytes).expect("parse for cache"));
    let cache: ResolverCache<u32> = ResolverCache::new(NonZeroUsize::new(16).expect("non-zero"));
    cache.insert(42, Arc::clone(&module));
    let key: u32 = 42;
    c.bench_function("cache_get_all_hit", |b| {
        b.iter(|| {
            let v = cache.get(black_box(&key)).expect("hit");
            black_box(v);
        });
    });
}

criterion_group!(benches, bench_parse, bench_resolve, bench_cache);
criterion_main!(benches);
