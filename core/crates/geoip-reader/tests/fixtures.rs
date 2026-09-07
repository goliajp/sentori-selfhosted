//! Integration tests through the public crate surface only.
//!
//! Each integration test owns its fixture builder (the internal
//! `crate::test_fixtures` module is `#[cfg(test)]`-private and
//! not reachable from `tests/`). Keeping a parallel builder is
//! the standard trade-off in Rust workspaces — it both covers
//! the public-API surface and catches re-export regressions.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::assigning_clones,
    missing_docs
)]

use std::collections::HashMap;
use std::net::IpAddr;

use maxminddb_writer::Database;
use maxminddb_writer::metadata::{IpVersion, Metadata};
use maxminddb_writer::paths::IpAddrWithMask;
use serde::Serialize;

use sentori_geoip_reader::{MmdbReader, ParseError};

#[derive(Serialize)]
struct CityRow {
    country: Country,
    city: City,
    subdivisions: Vec<Subdivision>,
    postal: Postal,
    location: Location,
}
#[derive(Serialize)]
struct Country {
    iso_code: String,
    names: Names,
    is_in_european_union: bool,
}
#[derive(Serialize)]
struct City {
    names: Names,
}
#[derive(Serialize)]
struct Subdivision {
    iso_code: String,
    names: Names,
}
#[derive(Serialize)]
struct Postal {
    code: String,
}
#[derive(Serialize)]
struct Location {
    latitude: f64,
    longitude: f64,
    accuracy_radius: u16,
}
#[derive(Serialize)]
struct Names {
    en: String,
}
#[derive(Serialize)]
struct AsnRow {
    autonomous_system_number: u32,
    autonomous_system_organization: String,
}

fn meta(database_type: &str) -> Metadata {
    let mut desc = HashMap::new();
    desc.insert("en".to_owned(), "integration fixture".to_owned());
    let mut m = Metadata::default();
    m.ip_version = IpVersion::V4;
    m.database_type = database_type.to_owned();
    m.languages = vec!["en".to_owned()];
    m.binary_format_major_version = 2;
    m.binary_format_minor_version = 0;
    m.build_epoch = 1_700_000_000;
    m.description = desc;
    m
}

fn city_db() -> Vec<u8> {
    let mut db = Database::default();
    db.metadata = meta("Sentori-City-Integration");
    let row = CityRow {
        country: Country {
            iso_code: "US".to_owned(),
            names: Names {
                en: "United States".to_owned(),
            },
            is_in_european_union: false,
        },
        city: City {
            names: Names {
                en: "San Francisco".to_owned(),
            },
        },
        subdivisions: vec![Subdivision {
            iso_code: "CA".to_owned(),
            names: Names {
                en: "California".to_owned(),
            },
        }],
        postal: Postal {
            code: "94103".to_owned(),
        },
        location: Location {
            latitude: 37.7749,
            longitude: -122.4194,
            accuracy_radius: 20,
        },
    };
    let data = db.insert_value(&row).expect("encode");
    let net: IpAddrWithMask = "192.0.2.0/24".parse().expect("net");
    db.insert_node(net, data);
    let mut out = Vec::new();
    db.write_to(&mut out).expect("write");
    out
}

fn asn_db() -> Vec<u8> {
    let mut db = Database::default();
    db.metadata = meta("Sentori-ASN-Integration");
    let row = AsnRow {
        autonomous_system_number: 15169,
        autonomous_system_organization: "GOOGLE".to_owned(),
    };
    let data = db.insert_value(&row).expect("encode");
    let net: IpAddrWithMask = "192.0.2.0/24".parse().expect("net");
    db.insert_node(net, data);
    let mut out = Vec::new();
    db.write_to(&mut out).expect("write");
    out
}

#[test]
fn city_lookup_via_public_api() {
    let bytes = city_db();
    let r = MmdbReader::from_bytes(bytes).expect("parse");
    let ip: IpAddr = "192.0.2.42".parse().expect("ip");
    let city = r.lookup_city(ip).expect("hit");
    assert_eq!(city.country.iso_code.as_deref(), Some("US"));
    assert_eq!(city.country.name_en.as_deref(), Some("United States"));
    assert_eq!(city.region_iso_code.as_deref(), Some("CA"));
    assert_eq!(city.region_name_en.as_deref(), Some("California"));
    assert_eq!(city.city_name_en.as_deref(), Some("San Francisco"));
    assert_eq!(city.postal_code.as_deref(), Some("94103"));
    let loc = city.location.expect("location");
    assert!((loc.latitude - 37.7749).abs() < 0.0001);
    assert!((loc.longitude - (-122.4194)).abs() < 0.0001);
    assert_eq!(loc.accuracy_radius_km, Some(20));
}

#[test]
fn asn_lookup_via_public_api() {
    let bytes = asn_db();
    let r = MmdbReader::from_bytes(bytes).expect("parse");
    let ip: IpAddr = "192.0.2.123".parse().expect("ip");
    let asn = r.lookup_asn(ip).expect("hit");
    assert_eq!(asn.asn, Some(15169));
    assert_eq!(asn.organisation.as_deref(), Some("GOOGLE"));
}

#[test]
fn unknown_ip_returns_none() {
    let bytes = city_db();
    let r = MmdbReader::from_bytes(bytes).expect("parse");
    // 203.0.113.0/24 is TEST-NET-3 — guaranteed unmapped.
    let ip: IpAddr = "203.0.113.42".parse().expect("ip");
    assert!(r.lookup_city(ip).is_none());
    assert!(r.lookup_country(ip).is_none());
    assert!(r.lookup_asn(ip).is_none());
}

#[test]
fn parse_error_on_garbage() {
    let err = MmdbReader::from_bytes(vec![1, 2, 3, 4]).expect_err("garbage");
    assert!(matches!(err, ParseError::InvalidDatabase(_)));
}

#[test]
fn metadata_visible_via_public_api() {
    let r = MmdbReader::from_bytes(city_db()).expect("parse");
    assert_eq!(r.database_type(), "Sentori-City-Integration");
    assert_eq!(r.ip_version(), 4);
    assert_eq!(r.build_epoch(), 1_700_000_000);
}

#[test]
fn reader_is_send_sync_via_arc() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<MmdbReader>();
    assert_sync::<MmdbReader>();
    assert_send::<std::sync::Arc<MmdbReader>>();
    assert_sync::<std::sync::Arc<MmdbReader>>();
}
