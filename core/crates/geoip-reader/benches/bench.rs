//! Criterion benches for the geoip-reader stone.
//!
//! Two hot paths:
//!
//! 1. `lookup_country_hit` — repeated `lookup_country` on an
//!    IP that resolves. Measures the steady-state cost of the
//!    decode + projection path.
//! 2. `lookup_country_miss` — repeated `lookup_country` on an
//!    unmapped IP. Measures the cost of the "early return None"
//!    path; should be cheaper.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::assigning_clones,
    missing_docs
)]

use std::collections::HashMap;
use std::hint::black_box;
use std::net::{IpAddr, Ipv4Addr};

use criterion::{Criterion, criterion_group, criterion_main};
use maxminddb_writer::Database;
use maxminddb_writer::metadata::{IpVersion, Metadata};
use maxminddb_writer::paths::IpAddrWithMask;
use sentori_geoip_reader::MmdbReader;
use serde::Serialize;

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

fn build_db() -> Vec<u8> {
    let mut m = Metadata::default();
    m.ip_version = IpVersion::V4;
    m.database_type = "Bench-Country".to_owned();
    m.languages = vec!["en".to_owned()];
    m.binary_format_major_version = 2;
    m.binary_format_minor_version = 0;
    m.build_epoch = 1_700_000_000;
    m.description = HashMap::new();
    let mut db = Database::default();
    db.metadata = m;
    let row = CountryRow {
        country: CountryInner {
            iso_code: "JP".to_owned(),
            names: Names {
                en: "Japan".to_owned(),
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

fn bench_lookup_hit(c: &mut Criterion) {
    let reader = MmdbReader::from_bytes(build_db()).expect("parse");
    let ip = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 42));
    c.bench_function("lookup_country_hit", |b| {
        b.iter(|| {
            let v = reader.lookup_country(black_box(ip));
            black_box(v);
        });
    });
}

fn bench_lookup_miss(c: &mut Criterion) {
    let reader = MmdbReader::from_bytes(build_db()).expect("parse");
    let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 42)); // TEST-NET-3
    c.bench_function("lookup_country_miss", |b| {
        b.iter(|| {
            let v = reader.lookup_country(black_box(ip));
            black_box(v);
        });
    });
}

criterion_group!(benches, bench_lookup_hit, bench_lookup_miss);
criterion_main!(benches);
