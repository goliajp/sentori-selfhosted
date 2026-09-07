//! Mach-O architecture identifiers.
//!
//! A fat (universal) Mach-O carries one slice per architecture; the
//! caller needs to tell the slicer which one to extract. We expose
//! a small typed enum rather than passing `(cputype, cpusubtype)`
//! tuples around — the four arches below cover ~100 % of what a
//! contemporary iOS / macOS / Catalyst build emits, and unknowns
//! fall through to [`Arch::Other`] with the raw type pair retained.
//!
//! `cputype` / `cpusubtype` values come from `<mach/machine.h>`;
//! the constants are reproduced here so we don't drag `object`'s
//! internal mach constants into the public API.

use core::fmt;

/// CPU architecture identifier carried in a Mach-O header.
///
/// The named variants match what Apple-platform toolchains emit
/// today. [`Arch::Other`] preserves the raw `(cputype, cpusubtype)`
/// pair for forward compatibility — e.g. future Apple Silicon
/// revisions or platform-specific subtypes we have not enumerated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arch {
    /// Apple Silicon (M-series, A-series 64-bit) — `arm64`.
    Arm64,
    /// Apple Silicon (M-series) with the e (extended) subtype —
    /// `arm64e`, used by Apple's pointer-authenticated binaries.
    Arm64e,
    /// 64-bit Intel — `x86_64`.
    X86_64,
    /// 32-bit ARM — `armv7`. Used by older iOS devices.
    Armv7,
    /// Any arch the named variants do not cover.
    Other {
        /// `cputype` from the Mach-O header.
        cputype: u32,
        /// `cpusubtype` from the Mach-O header.
        cpusubtype: u32,
    },
}

// `cputype` constants — masked to drop the 64-bit ABI bit when
// matching the named variants.
const CPU_TYPE_X86_64: u32 = 0x0100_0007;
const CPU_TYPE_ARM: u32 = 0x0000_000c;
const CPU_TYPE_ARM64: u32 = 0x0100_000c;

// `cpusubtype` values we care about — masked to drop the
// `CPU_SUBTYPE_LIB64` / `CPU_SUBTYPE_PTRAUTH_ABI` bits at the top.
const SUBTYPE_MASK: u32 = 0x00ff_ffff;
const CPU_SUBTYPE_ARM64_ALL: u32 = 0;
const CPU_SUBTYPE_ARM64E: u32 = 2;
const CPU_SUBTYPE_ARM_V7: u32 = 9;

impl Arch {
    /// Map a raw `(cputype, cpusubtype)` pair to a typed [`Arch`].
    ///
    /// Subtype bits above the platform field are masked off before
    /// matching, so a `CPU_SUBTYPE_PTRAUTH_ABI`-tagged `arm64e`
    /// still resolves to [`Arch::Arm64e`].
    #[must_use]
    pub const fn from_raw(cputype: u32, cpusubtype: u32) -> Self {
        let st = cpusubtype & SUBTYPE_MASK;
        match (cputype, st) {
            (CPU_TYPE_ARM64, CPU_SUBTYPE_ARM64E) => Self::Arm64e,
            (CPU_TYPE_ARM64, _) => Self::Arm64,
            (CPU_TYPE_X86_64, _) => Self::X86_64,
            (CPU_TYPE_ARM, CPU_SUBTYPE_ARM_V7) => Self::Armv7,
            _ => Self::Other {
                cputype,
                cpusubtype,
            },
        }
    }

    /// The named string Apple's `lipo -archs` uses for this arch.
    ///
    /// Returns the canonical form (lowercase, no spaces) for the
    /// four named variants and a `cputype:cpusubtype` decimal pair
    /// for [`Arch::Other`].
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Arm64 => "arm64".to_owned(),
            Self::Arm64e => "arm64e".to_owned(),
            Self::X86_64 => "x86_64".to_owned(),
            Self::Armv7 => "armv7".to_owned(),
            Self::Other {
                cputype,
                cpusubtype,
            } => format!("cpu:{cputype}/sub:{cpusubtype}"),
        }
    }

    /// The `cputype` field this arch lands in.
    #[must_use]
    pub const fn cputype(&self) -> u32 {
        match self {
            Self::Arm64 | Self::Arm64e => CPU_TYPE_ARM64,
            Self::X86_64 => CPU_TYPE_X86_64,
            Self::Armv7 => CPU_TYPE_ARM,
            Self::Other { cputype, .. } => *cputype,
        }
    }

    /// The `cpusubtype` field this arch lands in (masked to the
    /// platform-portion of the field — the high bits like
    /// `CPU_SUBTYPE_PTRAUTH_ABI` are not synthesised here).
    #[must_use]
    pub const fn cpusubtype(&self) -> u32 {
        match self {
            Self::Arm64 => CPU_SUBTYPE_ARM64_ALL,
            Self::Arm64e => CPU_SUBTYPE_ARM64E,
            Self::X86_64 => 3, // CPU_SUBTYPE_X86_64_ALL
            Self::Armv7 => CPU_SUBTYPE_ARM_V7,
            Self::Other { cpusubtype, .. } => *cpusubtype,
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
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

    #[test]
    fn round_trip_arm64() {
        let a = Arch::from_raw(CPU_TYPE_ARM64, CPU_SUBTYPE_ARM64_ALL);
        assert_eq!(a, Arch::Arm64);
        assert_eq!(a.name(), "arm64");
        assert_eq!(a.cputype(), CPU_TYPE_ARM64);
    }

    #[test]
    fn round_trip_arm64e_strips_high_bits() {
        // PTRAUTH_ABI flag sets the top bit of cpusubtype; mask must
        // drop it so we still recognise arm64e.
        let raw_subtype = CPU_SUBTYPE_ARM64E | 0x8000_0000;
        let a = Arch::from_raw(CPU_TYPE_ARM64, raw_subtype);
        assert_eq!(a, Arch::Arm64e);
    }

    #[test]
    fn round_trip_x86_64() {
        let a = Arch::from_raw(CPU_TYPE_X86_64, 3);
        assert_eq!(a, Arch::X86_64);
        assert_eq!(a.name(), "x86_64");
    }

    #[test]
    fn round_trip_armv7() {
        let a = Arch::from_raw(CPU_TYPE_ARM, CPU_SUBTYPE_ARM_V7);
        assert_eq!(a, Arch::Armv7);
        assert_eq!(a.name(), "armv7");
    }

    #[test]
    fn unknown_falls_through_to_other() {
        let a = Arch::from_raw(0xdead_beef, 0x42);
        assert_eq!(
            a,
            Arch::Other {
                cputype: 0xdead_beef,
                cpusubtype: 0x42
            }
        );
        let n = a.name();
        assert!(n.starts_with("cpu:") && n.contains("/sub:"), "got: {n}");
    }

    #[test]
    fn display_matches_name() {
        let a = Arch::Arm64;
        assert_eq!(format!("{a}"), "arm64");
    }
}
