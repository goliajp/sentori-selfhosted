//! Synthetic Mach-O + DWARF fixtures for the crate's own tests
//! plus the integration test suite under `tests/`.
//!
//! Built end-to-end with `gimli::write` (DWARF emission) +
//! `object::write` (Mach-O wrapping). No external toolchain
//! (`dsymutil`, `clang`, …) and no committed binary blobs — the
//! fixtures regenerate from source every test run, identical on
//! Linux CI and macOS dev machines.
//!
//! The shapes we synthesise are deliberately minimal: one CU, one
//! subprogram, one source line. That's the smallest input that
//! exercises addr2line's three real steps (find CU containing PC,
//! resolve name from DIE tree, resolve file:line from line
//! program); anything richer should live in dedicated end-to-end
//! tests, not here.

#![cfg(test)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::option_if_let_else,
    missing_docs
)]

use gimli::write::{Address, AttributeValue, EndianVec, FileId, LineProgram, LineString, Sections};
use gimli::{DW_AT_comp_dir, DW_AT_decl_file, DW_AT_decl_line, DW_AT_high_pc, DW_AT_language};
use gimli::{DW_AT_low_pc, DW_AT_name, DW_LANG_Rust, DW_TAG_subprogram};
use gimli::{Encoding, Format, LineEncoding, LittleEndian, SectionId};
use object::write::{Object, StandardSegment, Symbol, SymbolSection};
use object::{
    Architecture, BinaryFormat, Endianness, SectionKind, SymbolFlags, SymbolKind, SymbolScope,
};

/// A built fixture plus the metadata a test needs to assert on it.
pub(crate) struct Fixture {
    pub bytes: Vec<u8>,
    /// The static offset (PC - image_base) of `known_function`'s
    /// first instruction. Asserting `resolve(known_offset)` returns
    /// `known_function` validates the full addr2line round trip.
    pub known_offset: u64,
    pub known_function: &'static str,
    pub known_file: &'static str,
    pub known_line: u32,
}

const FUNCTION_NAME: &str = "hello";
const FILE_NAME: &str = "src/hello.rs";
const COMP_DIR: &str = "/build";
const DECL_LINE: u32 = 42;
const FUNCTION_LOW_PC: u64 = 0x1000;
const FUNCTION_LEN: u64 = 0x80;

/// Build a single-arch arm64 Mach-O carrying DWARF that describes
/// one subprogram (`hello` at `src/hello.rs:42`, occupying the
/// address range `0x1000..0x1080`).
pub(crate) fn synthetic_macho_with_dwarf() -> Fixture {
    let dwarf_sections = build_dwarf_sections();
    let bytes = wrap_in_macho(dwarf_sections);
    Fixture {
        bytes,
        known_offset: FUNCTION_LOW_PC,
        known_function: FUNCTION_NAME,
        known_file: FILE_NAME,
        known_line: DECL_LINE,
    }
}

/// A second fixture variant: same shape but compiled as x86_64.
/// Used by the slicer round-trip tests so a fat header can carry
/// two distinct slices.
pub(crate) fn synthetic_macho_x86_64() -> Fixture {
    let dwarf_sections = build_dwarf_sections();
    let bytes = wrap_in_macho_with_arch(dwarf_sections, Architecture::X86_64);
    Fixture {
        bytes,
        known_offset: FUNCTION_LOW_PC,
        known_function: FUNCTION_NAME,
        known_file: FILE_NAME,
        known_line: DECL_LINE,
    }
}

/// Pack the given (arch, bytes) pairs into a fat (32-bit header)
/// Mach-O. Used by [`crate::macho::tests`] to exercise the slicer
/// without committing a real fat dSYM blob to the repo.
///
/// Layout (big-endian on disk):
///
/// ```text
///   u32 magic       = FAT_MAGIC (0xCAFEBABE)
///   u32 nfat_arch
///   N × FatArch32 { cputype, cpusubtype, offset, size, align }
///   per-arch payload bytes (aligned to 2^align bytes)
/// ```
pub(crate) fn build_fat32(slices: &[(crate::arch::Arch, &[u8])]) -> Vec<u8> {
    use object::macho::FAT_MAGIC;

    let header_len = 8usize;
    let arch_len = 20usize;
    let n = u32::try_from(slices.len()).expect("at most u32::MAX slices");

    // Lay out payloads after the headers, aligned to 2^12 — the
    // boundary Apple uses for fat slices.
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

    out[..4].copy_from_slice(&FAT_MAGIC.to_be_bytes());
    out[4..8].copy_from_slice(&n.to_be_bytes());

    for (i, ((arch, payload), &offset)) in slices.iter().zip(payload_offsets.iter()).enumerate() {
        let entry_start = header_len + i * arch_len;
        out[entry_start..entry_start + 4].copy_from_slice(&arch.cputype().to_be_bytes());
        out[entry_start + 4..entry_start + 8].copy_from_slice(&arch.cpusubtype().to_be_bytes());
        out[entry_start + 8..entry_start + 12].copy_from_slice(
            &u32::try_from(offset)
                .expect("fat32 offset fits u32")
                .to_be_bytes(),
        );
        out[entry_start + 12..entry_start + 16].copy_from_slice(
            &u32::try_from(payload.len())
                .expect("payload size fits u32")
                .to_be_bytes(),
        );
        out[entry_start + 16..entry_start + 20].copy_from_slice(&align_pow.to_be_bytes());

        let offset_usize = offset as usize;
        out[offset_usize..offset_usize + payload.len()].copy_from_slice(payload);
    }

    out
}

/// A Mach-O with no DWARF — used to drive the "stripped object"
/// rejection path.
#[allow(dead_code)]
pub(crate) fn synthetic_macho_stripped() -> Vec<u8> {
    let mut obj = Object::new(
        BinaryFormat::MachO,
        Architecture::Aarch64,
        Endianness::Little,
    );
    add_text_padding(&mut obj);
    obj.write().expect("write stripped macho")
}

fn build_dwarf_sections() -> gimli::write::Dwarf {
    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 4,
        address_size: 8,
    };

    let mut dwarf = gimli::write::Dwarf::new();
    let line_program = make_line_program(encoding);
    let mut unit = gimli::write::Unit::new(encoding, line_program);

    // Register file into the line program table.
    let comp_dir_dir = unit
        .line_program
        .add_directory(LineString::String(COMP_DIR.as_bytes().to_vec()));
    let file_id = unit.line_program.add_file(
        LineString::String(FILE_NAME.as_bytes().to_vec()),
        comp_dir_dir,
        None,
    );

    let comp_dir_str_id = dwarf.strings.add(COMP_DIR);
    let cu_name_id = dwarf.strings.add(FILE_NAME);
    let function_name_id = dwarf.strings.add(FUNCTION_NAME);

    let root = unit.root();
    let root_die = unit.get_mut(root);
    root_die.set(DW_AT_comp_dir, AttributeValue::StringRef(comp_dir_str_id));
    root_die.set(DW_AT_name, AttributeValue::StringRef(cu_name_id));
    root_die.set(DW_AT_language, AttributeValue::Language(DW_LANG_Rust));
    root_die.set(
        DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(FUNCTION_LOW_PC)),
    );
    root_die.set(DW_AT_high_pc, AttributeValue::Udata(FUNCTION_LEN));

    // Subprogram DIE.
    let subprogram = unit.add(root, DW_TAG_subprogram);
    let sub_die = unit.get_mut(subprogram);
    sub_die.set(DW_AT_name, AttributeValue::StringRef(function_name_id));
    sub_die.set(
        DW_AT_low_pc,
        AttributeValue::Address(Address::Constant(FUNCTION_LOW_PC)),
    );
    sub_die.set(DW_AT_high_pc, AttributeValue::Udata(FUNCTION_LEN));
    sub_die.set(DW_AT_decl_file, AttributeValue::FileIndex(Some(file_id)));
    sub_die.set(DW_AT_decl_line, AttributeValue::Udata(u64::from(DECL_LINE)));

    // Emit one line program row covering FUNCTION_LOW_PC → DECL_LINE.
    emit_line_program_rows(&mut unit, file_id);

    let _unit_id = dwarf.units.add(unit);
    dwarf
}

fn make_line_program(encoding: Encoding) -> LineProgram {
    // gimli 0.33 signature:
    //   new(encoding, line_encoding,
    //       working_dir: LineString,
    //       source_dir: Option<LineString>,
    //       source_file: LineString,
    //       source_file_info: Option<FileInfo>)
    LineProgram::new(
        encoding,
        LineEncoding::default(),
        LineString::String(COMP_DIR.as_bytes().to_vec()),
        None,
        LineString::String(FILE_NAME.as_bytes().to_vec()),
        None,
    )
}

fn emit_line_program_rows(unit: &mut gimli::write::Unit, file_id: FileId) {
    let lp = &mut unit.line_program;
    lp.begin_sequence(Some(Address::Constant(FUNCTION_LOW_PC)));
    lp.row().file = file_id;
    lp.row().line = u64::from(DECL_LINE);
    lp.row().address_offset = 0;
    lp.generate_row();
    lp.end_sequence(FUNCTION_LEN);
}

fn wrap_in_macho(dwarf: gimli::write::Dwarf) -> Vec<u8> {
    wrap_in_macho_with_arch(dwarf, Architecture::Aarch64)
}

fn wrap_in_macho_with_arch(mut dwarf: gimli::write::Dwarf, arch: Architecture) -> Vec<u8> {
    let mut obj = Object::new(BinaryFormat::MachO, arch, Endianness::Little);

    // Write the DWARF sections into the Mach-O. We allocate each
    // section under the `__DWARF` segment so addr2line's section-
    // name lookup ("__debug_info", etc.) finds them.
    let mut sections = Sections::new(EndianVec::new(LittleEndian));
    dwarf.write(&mut sections).expect("emit dwarf");

    sections
        .for_each(|id: SectionId, data| -> Result<(), ()> {
            if data.slice().is_empty() {
                return Ok(());
            }
            let name = macho_section_name(id);
            let segment = obj.segment_name(StandardSegment::Debug).to_vec();
            let sect_id = obj.add_section(segment, name.into_bytes(), SectionKind::Debug);
            obj.append_section_data(sect_id, data.slice(), 1);
            Ok(())
        })
        .expect("iterate dwarf sections");

    add_text_padding(&mut obj);
    obj.write().expect("write macho")
}

fn macho_section_name(id: SectionId) -> String {
    // Mach-O renames `.debug_*` → `__debug_*` by convention. The
    // `object` crate's section-name accessor exposes the renamed
    // form on read, so we emit the same on write.
    let raw = id.name();
    if let Some(stripped) = raw.strip_prefix(".debug_") {
        format!("__debug_{stripped}")
    } else if let Some(stripped) = raw.strip_prefix(".") {
        format!("__{stripped}")
    } else {
        raw.to_owned()
    }
}

/// Append a tiny `__text` section sized to cover the address range
/// referenced by the DWARF (`FUNCTION_LOW_PC..FUNCTION_LOW_PC +
/// FUNCTION_LEN`). Without it the Mach-O writer rejects the
/// reference as out of range.
fn add_text_padding(obj: &mut Object<'_>) {
    let text_seg = obj.segment_name(StandardSegment::Text).to_vec();
    let text_id = obj.add_section(text_seg, b"__text".to_vec(), SectionKind::Text);
    let total_len = (FUNCTION_LOW_PC + FUNCTION_LEN) as usize;
    let padding = vec![0u8; total_len];
    obj.append_section_data(text_id, &padding, 16);

    // Define a symbol at FUNCTION_LOW_PC so the resulting Mach-O
    // has a recognised entry point — purely defensive; addr2line's
    // CU walk doesn't need it but `object::File::parse` is happier
    // when the file has at least one symbol.
    obj.add_symbol(Symbol {
        name: b"_start".to_vec(),
        value: FUNCTION_LOW_PC,
        size: FUNCTION_LEN,
        kind: SymbolKind::Text,
        scope: SymbolScope::Linkage,
        weak: false,
        section: SymbolSection::Section(text_id),
        flags: SymbolFlags::None,
    });
}
