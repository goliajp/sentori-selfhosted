//! Mach-O slice extraction from `.dSYM` bundles.
//!
//! A `.dSYM` is a directory layout:
//!
//!     Foo.dSYM/Contents/Info.plist
//!     Foo.dSYM/Contents/Resources/DWARF/Foo
//!
//! The DWARF file is either a single-arch Mach-O or a fat
//! (universal) binary containing one slice per arch. Each slice has
//! an `LC_UUID` load command we read to produce the `debug_id` the
//! server keys on, plus a `cputype`/`cpusubtype` we map to a
//! human-readable arch string (`arm64`, `x86_64`, …) atos understands.
//!
//! For fat binaries we slice the buffer rather than the file so the
//! server only sees the bytes for the (uuid, arch) it's storing.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use object::macho::{
    CPU_SUBTYPE_ARM64E, CPU_SUBTYPE_X86_64_H, CPU_TYPE_ARM, CPU_TYPE_ARM64, CPU_TYPE_ARM64_32,
    CPU_TYPE_X86, CPU_TYPE_X86_64, MachHeader32, MachHeader64,
};
use object::read::macho::{FatArch, MachHeader, MachOFatFile32, MachOFatFile64};
use object::{Endianness, FileKind};

pub struct Slice {
    pub arch: String,
    pub data: Vec<u8>,
    pub debug_id: String,
    pub object_name: String,
    pub size_bytes: u64,
}

pub fn slices_from_bundle(bundle: &Path) -> Result<Vec<Slice>> {
    let dwarf_dir = bundle.join("Contents/Resources/DWARF");
    if !dwarf_dir.is_dir() {
        return Err(anyhow!(
            "{} does not look like a .dSYM (no Contents/Resources/DWARF)",
            bundle.display()
        ));
    }
    let mut slices = Vec::new();
    for entry in
        fs::read_dir(&dwarf_dir).with_context(|| format!("reading {}", dwarf_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let object_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let mut found = parse_macho(&bytes, &object_name)?;
        slices.append(&mut found);
    }
    Ok(slices)
}

fn parse_macho(bytes: &[u8], object_name: &str) -> Result<Vec<Slice>> {
    match FileKind::parse(bytes)? {
        FileKind::MachO32 => {
            let header = MachHeader32::<Endianness>::parse(bytes, 0)?;
            single_slice(bytes, header, object_name).map(|s| vec![s])
        }
        FileKind::MachO64 => {
            let header = MachHeader64::<Endianness>::parse(bytes, 0)?;
            single_slice(bytes, header, object_name).map(|s| vec![s])
        }
        FileKind::MachOFat32 => {
            let fat = MachOFatFile32::parse(bytes)?;
            fat.arches()
                .iter()
                .map(|a| {
                    let slice_bytes = a.data(bytes)?;
                    let header = MachHeader32::<Endianness>::parse(slice_bytes, 0)?;
                    single_slice(slice_bytes, header, object_name)
                })
                .collect()
        }
        FileKind::MachOFat64 => {
            let fat = MachOFatFile64::parse(bytes)?;
            fat.arches()
                .iter()
                .map(|a| {
                    let slice_bytes = a.data(bytes)?;
                    let header = MachHeader64::<Endianness>::parse(slice_bytes, 0)?;
                    single_slice(slice_bytes, header, object_name)
                })
                .collect()
        }
        other => Err(anyhow!("not a Mach-O file: {other:?}")),
    }
}

fn single_slice<H: MachHeader>(bytes: &[u8], header: &H, object_name: &str) -> Result<Slice> {
    let endian = header.endian()?;
    let mut commands = header.load_commands(endian, bytes, 0)?;
    let mut uuid: Option<[u8; 16]> = None;
    while let Some(cmd) = commands.next()? {
        if let Some(u) = cmd.uuid()? {
            uuid = Some(u.uuid);
            break;
        }
    }
    let uuid = uuid.ok_or_else(|| anyhow!("Mach-O slice has no LC_UUID"))?;

    Ok(Slice {
        arch: arch_name(header.cputype(endian), header.cpusubtype(endian)).to_string(),
        data: bytes.to_vec(),
        debug_id: format_uuid(&uuid),
        object_name: object_name.to_string(),
        size_bytes: bytes.len() as u64,
    })
}

fn format_uuid(b: &[u8; 16]) -> String {
    let h = b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

fn arch_name(cputype: u32, cpusubtype: u32) -> &'static str {
    // mask out the CPU_SUBTYPE_LIB64 bit; it's not part of the
    // identity we care about.
    let sub = cpusubtype & 0x00ff_ffff;
    match cputype {
        CPU_TYPE_ARM64 if sub == CPU_SUBTYPE_ARM64E => "arm64e",
        CPU_TYPE_ARM64 => "arm64",
        CPU_TYPE_ARM64_32 => "arm64_32",
        CPU_TYPE_ARM => match sub {
            6 => "armv6",
            9 => "armv7",
            11 => "armv7s",
            12 => "armv7k",
            _ => "arm",
        },
        CPU_TYPE_X86_64 if sub == CPU_SUBTYPE_X86_64_H => "x86_64h",
        CPU_TYPE_X86_64 => "x86_64",
        CPU_TYPE_X86 => "i386",
        _ => "unknown",
    }
}
