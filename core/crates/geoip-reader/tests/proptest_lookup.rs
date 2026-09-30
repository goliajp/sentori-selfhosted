//! Property tests for the .mmdb reader.
//!
//! Two invariants:
//!
//! 1. **Round-trip.** An IP inserted into the synthesised db
//!    always resolves back to the country code that was written.
//! 2. **Negative.** IPs in TEST-NET / private ranges that were
//!    not inserted always return `None`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::assigning_clones,
    missing_docs
)]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};

use maxminddb_writer::Database;
use maxminddb_writer::metadata::{IpVersion, Metadata};
use maxminddb_writer::paths::IpAddrWithMask;
use proptest::prelude::*;
use serde::Serialize;

use sentori_geoip_reader::MmdbReader;

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
struct Names {
    en: String,
}

fn meta() -> Metadata {
    let mut m = Metadata::default();
    m.ip_version = IpVersion::V4;
    m.database_type = "Proptest-Country".to_owned();
    m.languages = vec!["en".to_owned()];
    m.binary_format_major_version = 2;
    m.binary_format_minor_version = 0;
    m.build_epoch = 1_700_000_000;
    m.description = HashMap::new();
    m
}

/// Build a db with one /24 network at `192.0.2.0/24` mapped to
/// the given ISO code.
fn build_db(iso: &str) -> Vec<u8> {
    let mut db = Database::default();
    db.metadata = meta();
    let row = CountryRow {
        country: CountryInner {
            iso_code: iso.to_owned(),
            names: Names {
                en: format!("Country-{iso}"),
            },
            is_in_european_union: false,
        },
    };
    let data = db.insert_value(&row).expect("encode");
    let net: IpAddrWithMask = "192.0.2.0/24".parse().expect("net");
    db.insert_node(net, data);
    let mut out = Vec::new();
    db.write_to(&mut out).expect("write");
    out
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        .. ProptestConfig::default()
    })]

    /// Round-trip: any ISO-shaped code written to the fixture
    /// db is recoverable for every IP in the inserted /24.
    #[test]
    fn round_trip_country_iso(
        iso in "[A-Z]{2}",
        host in 0u8..=255,
    ) {
        let r = MmdbReader::from_bytes(build_db(&iso)).expect("parse");
        let ip = IpAddr::V4(Ipv4Addr::new(192, 0, 2, host));
        let country = r.lookup_country(ip).expect("hit");
        prop_assert_eq!(country.iso_code.as_deref(), Some(iso.as_str()));
    }

    /// Negative: IPs outside the inserted /24 always return None
    /// regardless of the iso code we wrote.
    #[test]
    fn outside_inserted_network_returns_none(
        iso in "[A-Z]{2}",
        a in 1u8..=255,
        b in 0u8..=255,
        c in 0u8..=255,
        d in 0u8..=255,
    ) {
        // Skip the inserted prefix 192.0.2.0/24 by ensuring (a,b,c)
        // isn't (192,0,2).
        prop_assume!(!(a == 192 && b == 0 && c == 2));
        let r = MmdbReader::from_bytes(build_db(&iso)).expect("parse");
        let ip = IpAddr::V4(Ipv4Addr::new(a, b, c, d));
        prop_assert!(r.lookup_country(ip).is_none());
    }
}
