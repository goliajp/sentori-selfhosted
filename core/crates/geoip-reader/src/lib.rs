//! # `sentori-geoip-reader` — MaxMind .mmdb (GeoIP2 / GeoLite2 / DB-IP Lite) reader
//!
//! Stone-tier crate (per cement-stone methodology) for IP →
//! geographic / ASN enrichment. Wraps the canonical Rust
//! `maxminddb` crate (ISC) with:
//!
//! - **A typed, owned surface.** `CountryRecord` / `CityRecord` /
//!   `AsnRecord` are owned-`String` projections — callers stash
//!   them straight into typed event structs with no lifetime
//!   parameters leaking through.
//! - **Bytes-in only.** [`MmdbReader::from_bytes`] takes a
//!   `Vec<u8>`; file I/O and memory-mapping live in the 钢筋
//!   layer (matches S7 `dwarf-resolver` and S8 `proguard-resolver`).
//! - **Three lookup methods.** `lookup_country` / `lookup_city`
//!   / `lookup_asn`. Each returns `Option<T>` — `None` on
//!   unmapped IPs (private / reserved range, db doesn't have a
//!   row) rather than synthesising an error, because geo
//!   enrichment is best-effort by convention.
//!
//! ## What this crate does NOT do
//!
//! - **No file I/O.** Callers pass bytes; the 钢筋 layer owns
//!   the path-to-bytes step (`std::fs::read`, S3 fetch, env-var
//!   resolution).
//! - **No memory-mapping.** The upstream `maxminddb::Reader`
//!   supports `mmap` via a feature, but the file handle would be
//!   K-tier concern.
//! - **No env-var integration.** The legacy `server/src/geoip.rs`
//!   read `SENTORI_GEOIP_DB_PATH`; that lives in the 钢筋 layer.
//! - **No caching.** The reader IS the cache — `Reader<Vec<u8>>`
//!   holds the parsed B-tree; one reader per db loaded at boot.
//!
//! ## Concurrency model
//!
//! [`MmdbReader`] is `Send + Sync` and lock-free for reads. Share
//! via `Arc<MmdbReader>` and call `lookup_*` from any thread.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use std::net::IpAddr;
//! use sentori_geoip_reader::MmdbReader;
//!
//! # fn demo(mmdb_bytes: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
//! let reader = MmdbReader::from_bytes(mmdb_bytes)?;
//! let ip: IpAddr = "203.0.113.42".parse()?;
//! if let Some(country) = reader.lookup_country(ip) {
//!     println!("{}", country.iso_code.unwrap_or_default());
//! }
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::multiple_crate_versions)]
// `pub(crate)` in a private module is the intended visibility —
// we use it deliberately so a future `pub use` doesn't widen
// access past `pub(crate)`.
#![allow(clippy::redundant_pub_crate)]

mod error;
mod reader;
mod record;

#[cfg(test)]
mod test_fixtures;

pub use error::{ParseError, ParseResult};
pub use reader::MmdbReader;
pub use record::{AsnRecord, CityRecord, CountryRecord, LatLong};
