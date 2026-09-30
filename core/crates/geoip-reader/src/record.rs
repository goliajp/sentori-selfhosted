//! Typed records — the owned-`String` projections this stone
//! returns from `lookup_*` calls.
//!
//! The upstream `maxminddb::geoip2::{Country, City, Asn}` types
//! deserialise from the .mmdb borrowed-`&str` fields. Our wrappers
//! copy to owned `String` so callers can stash records into typed
//! event structs without lifetime ceremony, the same shape S6
//! `Resolution` and S7 `Frame` use.

use serde::{Deserialize, Serialize};

/// Country-level enrichment record.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CountryRecord {
    /// ISO 3166-1 alpha-2 code (e.g. `"JP"`, `"US"`). `None` when
    /// the IP maps to a row without a country code (some private /
    /// reserved ranges).
    pub iso_code: Option<String>,
    /// English country name (e.g. `"Japan"`, `"United States"`).
    /// Other languages are deliberately not exposed — the v0.1
    /// dashboard is English-only; localisation is a 钢筋-layer
    /// concern.
    pub name_en: Option<String>,
    /// `true` iff the IP is mapped to an EU member state. Useful
    /// for GDPR-flavoured routing decisions.
    pub is_in_european_union: bool,
}

/// City-level enrichment record.
///
/// Superset of [`CountryRecord`]; if the underlying database only
/// has country precision (DB-IP Lite Country, GeoLite2 Country)
/// the city / region / location fields are simply `None`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CityRecord {
    /// Country-level slice of the record.
    pub country: CountryRecord,
    /// Subdivision (state / prefecture) ISO 3166-2 region code.
    /// `"US-CA"`-style suffix only — the country prefix lives in
    /// `country.iso_code`.
    pub region_iso_code: Option<String>,
    /// English subdivision name (e.g. `"California"`, `"Tokyo"`).
    pub region_name_en: Option<String>,
    /// English city name (e.g. `"San Francisco"`, `"Tokyo"`).
    pub city_name_en: Option<String>,
    /// Postal code, if the database carries it. Free-form string
    /// per MaxMind's schema.
    pub postal_code: Option<String>,
    /// Approximate geo coordinates of the city centroid, if the
    /// database carries them.
    pub location: Option<LatLong>,
}

/// Coarse geographic coordinates from a .mmdb city record.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct LatLong {
    /// Latitude in degrees.
    pub latitude: f64,
    /// Longitude in degrees.
    pub longitude: f64,
    /// Stated accuracy radius of the coordinate, in kilometres.
    /// `None` when the database does not record it.
    pub accuracy_radius_km: Option<u16>,
}

impl Eq for LatLong {}

/// ASN enrichment record.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AsnRecord {
    /// AS number (e.g. `15169` for Google).
    pub asn: Option<u32>,
    /// AS organisation name (e.g. `"GOOGLE"`).
    pub organisation: Option<String>,
}
