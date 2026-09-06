//! Synthesised .mmdb fixtures for the crate's own tests + the
//! integration suite under `tests/`.
//!
//! Built via `maxminddb-writer` at test time — no committed
//! binary blob, no GeoLite2 EULA snag, runs identical on Linux
//! CI and macOS dev. Same approach S7 took for Mach-O+DWARF via
//! `gimli::write` + `object::write`.

#![cfg(test)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    dead_code,
    missing_docs
)]

use std::collections::HashMap;
use std::net::IpAddr;

use maxminddb_writer::Database;
use maxminddb_writer::metadata::{IpVersion, Metadata};
use maxminddb_writer::paths::IpAddrWithMask;
use serde::Serialize;

/// IPv4 inside the `Country` fixture's `203.0.110.0/24` row.
/// Call sites parse on demand; cheaper than `OnceLock` for a
/// single use per test.
pub(crate) fn country_fixture_ip() -> IpAddr {
    "203.0.110.42".parse().expect("static ip")
}

/// IPv4 inside the `City` fixture's `203.0.111.0/24` row.
pub(crate) fn city_fixture_ip() -> IpAddr {
    "203.0.111.42".parse().expect("static ip")
}

/// IPv4 inside the `ASN` fixture's `203.0.112.0/24` row.
pub(crate) fn asn_fixture_ip() -> IpAddr {
    "203.0.112.42".parse().expect("static ip")
}

/// Build a synthetic country-precision .mmdb wired to map
/// `203.0.110.0/24` → `{ country: { iso_code: "JP", ... } }`.
pub(crate) fn build_country_fixture() -> Vec<u8> {
    let mut db = Database::default();
    db.metadata = country_metadata();

    let row = country_row("JP", "Japan", false);
    let data = db.insert_value(&row).expect("encode row");
    let net: IpAddrWithMask = "203.0.110.0/24".parse().expect("net");
    db.insert_node(net, data);

    let mut out = Vec::new();
    db.write_to(&mut out).expect("write mmdb");
    out
}

/// Build a synthetic city-precision .mmdb wired to map
/// `203.0.111.0/24` → a Tokyo, Kanto, Japan city record at
/// (35.6762, 139.6503) with accuracy 50 km.
pub(crate) fn build_city_fixture() -> Vec<u8> {
    let mut db = Database::default();
    db.metadata = city_metadata();

    let row = city_row();
    let data = db.insert_value(&row).expect("encode row");
    let net: IpAddrWithMask = "203.0.111.0/24".parse().expect("net");
    db.insert_node(net, data);

    let mut out = Vec::new();
    db.write_to(&mut out).expect("write mmdb");
    out
}

/// Build a synthetic ASN .mmdb wired to map `203.0.112.0/24` →
/// `{ autonomous_system_number: 15169, autonomous_system_organization: "GOOGLE" }`.
pub(crate) fn build_asn_fixture() -> Vec<u8> {
    let mut db = Database::default();
    db.metadata = asn_metadata();

    let row = asn_row(15169, "GOOGLE");
    let data = db.insert_value(&row).expect("encode row");
    let net: IpAddrWithMask = "203.0.112.0/24".parse().expect("net");
    db.insert_node(net, data);

    let mut out = Vec::new();
    db.write_to(&mut out).expect("write mmdb");
    out
}

// `Metadata`'s `node_count` / `record_size` fields are
// pub(crate); we have to construct via Default + mutation
// rather than FRU.

fn build_meta(database_type: &str, description: &str) -> Metadata {
    let mut desc = HashMap::new();
    desc.insert("en".to_owned(), description.to_owned());
    let mut meta = Metadata::default();
    // V4 to match the IPv4 paths we insert below. Real MaxMind
    // drops use V6 with v4 mapped under `::ffff:0:0/96`; we
    // could mirror that but it complicates the fixture without
    // adding test coverage (the reader's behaviour is identical).
    meta.ip_version = IpVersion::V4;
    meta.database_type = database_type.to_owned();
    meta.languages = vec!["en".to_owned()];
    meta.binary_format_major_version = 2;
    meta.binary_format_minor_version = 0;
    meta.build_epoch = 1_700_000_000;
    meta.description = desc;
    meta
}

fn country_metadata() -> Metadata {
    build_meta("Sentori-Country-Test", "Synthetic country fixture")
}

fn city_metadata() -> Metadata {
    build_meta("Sentori-City-Test", "Synthetic city fixture")
}

fn asn_metadata() -> Metadata {
    build_meta("Sentori-ASN-Test", "Synthetic ASN fixture")
}

// ── Wire-format shapes (mirror `maxminddb::geoip2`'s schema) ─

#[derive(Serialize)]
struct CountryRow {
    country: CountryInner,
}

#[derive(Serialize)]
struct CountryInner {
    iso_code: String,
    names: Names,
    is_in_european_union: bool,
}

#[derive(Serialize)]
struct CityRow {
    country: CountryInner,
    city: CityInner,
    subdivisions: Vec<SubdivisionInner>,
    postal: PostalInner,
    location: LocationInner,
}

#[derive(Serialize)]
struct CityInner {
    names: Names,
}

#[derive(Serialize)]
struct SubdivisionInner {
    iso_code: String,
    names: Names,
}

#[derive(Serialize)]
struct PostalInner {
    code: String,
}

#[derive(Serialize)]
struct LocationInner {
    latitude: f64,
    longitude: f64,
    accuracy_radius: u16,
}

#[derive(Serialize)]
struct AsnRow {
    autonomous_system_number: u32,
    autonomous_system_organization: String,
}

#[derive(Serialize)]
struct Names {
    en: String,
}

fn country_row(iso: &str, name_en: &str, eu: bool) -> CountryRow {
    CountryRow {
        country: CountryInner {
            iso_code: iso.to_owned(),
            names: Names {
                en: name_en.to_owned(),
            },
            is_in_european_union: eu,
        },
    }
}

fn city_row() -> CityRow {
    CityRow {
        country: CountryInner {
            iso_code: "JP".to_owned(),
            names: Names {
                en: "Japan".to_owned(),
            },
            is_in_european_union: false,
        },
        city: CityInner {
            names: Names {
                en: "Tokyo".to_owned(),
            },
        },
        subdivisions: vec![SubdivisionInner {
            iso_code: "13".to_owned(),
            names: Names {
                en: "Tokyo".to_owned(),
            },
        }],
        postal: PostalInner {
            code: "100-0001".to_owned(),
        },
        location: LocationInner {
            latitude: 35.6762,
            longitude: 139.6503,
            accuracy_radius: 50,
        },
    }
}

fn asn_row(asn: u32, org: &str) -> AsnRow {
    AsnRow {
        autonomous_system_number: asn,
        autonomous_system_organization: org.to_owned(),
    }
}
