//! End-to-end roundtrip test exercising the public crate surface
//! without poking at internal modules.
//!
//! We rebuild the synthesised Mach-O fixture here (the
//! `test_fixtures` module is `#[cfg(test)]` internal — integration
//! tests need their own fixture builder) and assert the same shape
//! the internal `module::tests` already cover, but through the
//! public API exclusively. Lets us catch any future regression that
//! breaks re-exports without breaking internal tests.

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

use gimli::write::{Address, AttributeValue, EndianVec, LineProgram, LineString, Sections};
use gimli::{DW_AT_comp_dir, DW_AT_decl_file, DW_AT_decl_line, DW_AT_high_pc, DW_AT_language};
use gimli::{DW_AT_low_pc, DW_AT_name, DW_LANG_Rust, DW_TAG_subprogram};
use gimli::{Encoding, Format, LineEncoding, LittleEndian, SectionId};
use object::write::{Object, StandardSegment, Symbol, SymbolSection};
use object::{
    Architecture, BinaryFormat, Endianness, SectionKind, SymbolFlags, SymbolKind, SymbolScope,
};

use sentori_dwarf_resolver::{Arch, DwarfModule, ParseError, ResolverCache};

const FUNCTION_NAME: &str = "answer";
const FILE_NAME: &str = "src/lib.rs";
const COMP_DIR: &str = "/proj";
const LOW_PC: u64 = 0x4000;
const FN_LEN: u64 = 0x40;
const DECL_LINE: u32 = 17;

fn synth_bytes() -> Vec<u8> {
    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 4,
        address_size: 8,
    };
    let mut dwarf = gimli::write::Dwarf::new();

    let line_program = LineProgram::new(
        encoding,
        LineEncoding::default(),
        LineString::String(COMP_DIR.as_bytes().to_vec()),
        None,
        LineString::String(FILE_NAME.as_bytes().to_vec()),
        None,
    );
    let mut unit = gimli::write::Unit::new(encoding, line_program);

    let dir = unit
        .line_program
        .add_directory(LineString::String(COMP_DIR.as_bytes().to_vec()));
    let file_id =
        unit.line_program
            .add_file(LineString::String(FILE_NAME.as_bytes().to_vec()), dir, None);

    let cd = dwarf.strings.add(COMP_DIR);
    let nm = dwarf.strings.add(FILE_NAME);
    let fn_nm = dwarf.strings.add(FUNCTION_NAME);

    let root = unit.root();
    let root_die = unit.get_mut(root);
    root_die.set(DW_AT_comp_dir, AttributeValue::StringRef(cd));
    root_die.set(DW_AT_name, AttributeValue::StringRef(nm));
    root_die.set(DW_AT_language, AttributeValue::Language(DW_LANG_Rust));
    root_die.set(
        DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(LOW_PC)),
    );
    root_die.set(DW_AT_high_pc, AttributeValue::Udata(FN_LEN));

    let sub = unit.add(root, DW_TAG_subprogram);
    let sub_die = unit.get_mut(sub);
    sub_die.set(DW_AT_name, AttributeValue::StringRef(fn_nm));
    sub_die.set(
        DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(LOW_PC)),
    );
    sub_die.set(DW_AT_high_pc, AttributeValue::Udata(FN_LEN));
    sub_die.set(DW_AT_decl_file, AttributeValue::FileIndex(Some(file_id)));
    sub_die.set(DW_AT_decl_line, AttributeValue::Udata(u64::from(DECL_LINE)));

    let lp = &mut unit.line_program;
    lp.begin_sequence(Some(Address::Constant(LOW_PC)));
    lp.row().file = file_id;
    lp.row().line = u64::from(DECL_LINE);
    lp.row().address_offset = 0;
    lp.generate_row();
    lp.end_sequence(FN_LEN);

    dwarf.units.add(unit);

    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections).expect("emit dwarf");

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
            let macho_name = raw
                .strip_prefix(".debug_")
                .map_or_else(|| raw.to_owned(), |s| format!("__debug_{s}"));
            let seg = obj.segment_name(StandardSegment::Debug).to_vec();
            let sect = obj.add_section(seg, macho_name.into_bytes(), SectionKind::Debug);
            obj.append_section_data(sect, data.slice(), 1);
            Ok(())
        })
        .expect("iterate sections");

    let total = (LOW_PC + FN_LEN) as usize;
    let text_seg = obj.segment_name(StandardSegment::Text).to_vec();
    let text = obj.add_section(text_seg, b"__text".to_vec(), SectionKind::Text);
    let padding = vec![0u8; total];
    obj.append_section_data(text, &padding, 16);
    obj.add_symbol(Symbol {
        name: b"_answer".to_vec(),
        value: LOW_PC,
        size: FN_LEN,
        kind: SymbolKind::Text,
        scope: SymbolScope::Linkage,
        weak: false,
        section: SymbolSection::Section(text),
        flags: SymbolFlags::None,
    });

    obj.write().expect("write macho")
}

#[test]
fn end_to_end_via_public_api() {
    let bytes = synth_bytes();
    let module = DwarfModule::from_bytes(bytes).expect("parse");

    let frames = module.resolve(LOW_PC).expect("resolve");
    assert!(!frames.is_empty());

    let leaf = &frames[0];
    assert_eq!(leaf.function.as_deref(), Some(FUNCTION_NAME));
    let f = leaf.file.as_deref().expect("file");
    assert!(f.ends_with(FILE_NAME), "got {f:?}");
    assert_eq!(leaf.line, Some(DECL_LINE));
}

#[test]
fn cache_round_trip_via_public_api() {
    let bytes = synth_bytes();
    let cache: ResolverCache<(String, Arch)> =
        ResolverCache::new(NonZeroUsize::new(2).expect("non-zero"));
    let key = (
        "BD93D1D5-A5C3-3A6D-87B1-DEADBEEF1234".to_owned(),
        Arch::Arm64,
    );

    let module = cache
        .get_or_try_insert_with::<_, ParseError>(&key, || {
            DwarfModule::from_bytes(bytes.clone()).map(Arc::new)
        })
        .expect("load");
    assert!(cache.get(&key).is_some(), "cache hit after load");

    let frames = module.resolve(LOW_PC).expect("resolve");
    assert_eq!(
        frames.first().and_then(|f| f.function.as_deref()),
        Some(FUNCTION_NAME)
    );
}

#[test]
fn rejects_stripped_object() {
    // A Mach-O with only a __text section and no DWARF — exercises
    // the NoDwarfSections branch via the public API.
    let mut obj = Object::new(
        BinaryFormat::MachO,
        Architecture::Aarch64,
        Endianness::Little,
    );
    let seg = obj.segment_name(StandardSegment::Text).to_vec();
    let text = obj.add_section(seg, b"__text".to_vec(), SectionKind::Text);
    obj.append_section_data(text, &[0u8; 128], 16);
    let bytes = obj.write().expect("write");

    let err = DwarfModule::from_bytes(bytes).expect_err("stripped");
    assert!(matches!(err, ParseError::NoDwarfSections), "got {err:?}");
}
