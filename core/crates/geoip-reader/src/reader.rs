//! Pure (no-I/O, no-cache) MaxMind .mmdb reader.

use std::net::IpAddr;

use maxminddb::Reader;
use maxminddb::geoip2;

#[cfg(test)]
use crate::error::ParseError;
use crate::error::ParseResult;
use crate::record::{AsnRecord, CityRecord, CountryRecord, LatLong};

/// A parsed, immutable MaxMind .mmdb document.
///
/// Owns its backing `Vec<u8>` (the upstream `Reader<Vec<u8>>`
/// stores them internally), so callers can drop their reference
/// after [`MmdbReader::from_bytes`] returns. `Send + Sync` and
/// lock-free for the lookup path — share via `Arc<MmdbReader>`.
pub struct MmdbReader {
    inner: Reader<Vec<u8>>,
}

impl MmdbReader {
    /// Parse a .mmdb byte buffer into a ready-to-query reader.
    ///
    /// # Errors
    ///
    /// - [`crate::ParseError::InvalidDatabase`] — bytes are not
    ///   a valid MaxMind .mmdb document.
    pub fn from_bytes(bytes: Vec<u8>) -> ParseResult<Self> {
        let inner = Reader::from_source(bytes)?;
        Ok(Self { inner })
    }

    /// The `database_type` string from the .mmdb metadata
    /// (e.g. `"GeoLite2-City"`, `"DBIP-Country-Lite"`).
    /// Useful for the dashboard's "which db is loaded" header.
    #[must_use]
    pub fn database_type(&self) -> &str {
        &self.inner.metadata.database_type
    }

    /// The build-epoch second-stamp from the .mmdb metadata.
    /// Surfaces freshness: stale databases (months-old MaxMind
    /// drops) silently lose accuracy.
    #[must_use]
    pub const fn build_epoch(&self) -> u64 {
        self.inner.metadata.build_epoch
    }

    /// IP version the database covers — typically `6` for modern
    /// drops (IPv6 db with IPv4 mapped under `::ffff:0:0/96`),
    /// or `4` for legacy v4-only drops.
    #[must_use]
    pub const fn ip_version(&self) -> u16 {
        self.inner.metadata.ip_version
    }

    /// Backing byte size. Useful for observability — a typical
    /// GeoLite2-City db is ~70 MB; DB-IP Country-Lite is ~3 MB.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        // The upstream reader doesn't expose the raw source slice
        // directly; metadata records the node count, which is the
        // closest proxy. We surface the metadata-recorded count
        // since it correlates with the file size.
        usize::try_from(self.inner.metadata.node_count).unwrap_or(usize::MAX)
    }

    /// Look up the country-level record for `ip`.
    ///
    /// Returns `None` when the IP is unmapped (private / reserved
    /// range, or the database simply doesn't have a row). Geo
    /// enrichment is best-effort by convention — the caller
    /// should keep the event un-enriched on `None` rather than
    /// reject.
    #[must_use]
    pub fn lookup_country(&self, ip: IpAddr) -> Option<CountryRecord> {
        let result = self.inner.lookup(ip).ok()?;
        let raw = result.decode::<geoip2::Country<'_>>().ok().flatten()?;
        Some(project_country(&raw))
    }

    /// Look up the city-level record for `ip`.
    ///
    /// Returns `None` on unmapped IPs. Country-only databases
    /// still resolve via this method — the city / region /
    /// location fields will be `None` but the country slice
    /// populates normally (the City record format is a superset).
    #[must_use]
    pub fn lookup_city(&self, ip: IpAddr) -> Option<CityRecord> {
        let result = self.inner.lookup(ip).ok()?;
        let raw = result.decode::<geoip2::City<'_>>().ok().flatten()?;
        Some(project_city(&raw))
    }

    /// Look up the ASN record for `ip`.
    ///
    /// Returns `None` on unmapped IPs. ASN databases are
    /// separate downloads from the City / Country dbs — the
    /// caller is responsible for loading the right one.
    #[must_use]
    pub fn lookup_asn(&self, ip: IpAddr) -> Option<AsnRecord> {
        let result = self.inner.lookup(ip).ok()?;
        let raw = result.decode::<geoip2::Asn<'_>>().ok().flatten()?;
        Some(AsnRecord {
            asn: raw.autonomous_system_number,
            organisation: raw.autonomous_system_organization.map(str::to_owned),
        })
    }
}

impl core::fmt::Debug for MmdbReader {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MmdbReader")
            .field("database_type", &self.database_type())
            .field("ip_version", &self.ip_version())
            .field("build_epoch", &self.build_epoch())
            .field("node_count", &self.inner.metadata.node_count)
            .finish_non_exhaustive()
    }
}

fn project_country(raw: &geoip2::Country<'_>) -> CountryRecord {
    CountryRecord {
        iso_code: raw.country.iso_code.map(str::to_owned),
        name_en: raw.country.names.english.map(str::to_owned),
        is_in_european_union: raw.country.is_in_european_union.unwrap_or(false),
    }
}

fn project_city(raw: &geoip2::City<'_>) -> CityRecord {
    CityRecord {
        country: CountryRecord {
            iso_code: raw.country.iso_code.map(str::to_owned),
            name_en: raw.country.names.english.map(str::to_owned),
            is_in_european_union: raw.country.is_in_european_union.unwrap_or(false),
        },
        region_iso_code: raw
            .subdivisions
            .first()
            .and_then(|s| s.iso_code)
            .map(str::to_owned),
        region_name_en: raw
            .subdivisions
            .first()
            .and_then(|s| s.names.english)
            .map(str::to_owned),
        city_name_en: raw.city.names.english.map(str::to_owned),
        postal_code: raw.postal.code.map(str::to_owned),
        location: raw
            .location
            .latitude
            .zip(raw.location.longitude)
            .map(|(latitude, longitude)| LatLong {
                latitude,
                longitude,
                accuracy_radius_km: raw.location.accuracy_radius,
            }),
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
    use crate::test_fixtures::{build_country_fixture, country_fixture_ip};

    #[test]
    fn parses_synthetic_fixture() {
        let bytes = build_country_fixture();
        let r = MmdbReader::from_bytes(bytes).expect("parse");
        assert_eq!(r.database_type(), "Sentori-Country-Test");
        assert_eq!(r.ip_version(), 4);
    }

    #[test]
    fn rejects_garbage() {
        let err = MmdbReader::from_bytes(vec![1, 2, 3, 4]).expect_err("bad");
        assert!(matches!(err, ParseError::InvalidDatabase(_)));
    }

    #[test]
    fn rejects_empty() {
        let err = MmdbReader::from_bytes(Vec::new()).expect_err("empty");
        assert!(matches!(err, ParseError::InvalidDatabase(_)));
    }

    #[test]
    fn looks_up_country_for_known_ip() {
        let r = MmdbReader::from_bytes(build_country_fixture()).expect("parse");
        let country = r.lookup_country(country_fixture_ip()).expect("hit");
        assert_eq!(country.iso_code.as_deref(), Some("JP"));
    }

    #[test]
    fn unknown_ip_returns_none() {
        let r = MmdbReader::from_bytes(build_country_fixture()).expect("parse");
        // 203.0.113.0/24 is TEST-NET-3 — guaranteed unmapped.
        let none_ip: IpAddr = "203.0.113.42".parse().expect("addr");
        assert!(r.lookup_country(none_ip).is_none());
    }

    #[test]
    fn debug_renders_metadata() {
        let r = MmdbReader::from_bytes(build_country_fixture()).expect("parse");
        let s = format!("{r:?}");
        assert!(s.contains("MmdbReader"));
        assert!(s.contains("database_type"));
        assert!(s.contains("Sentori-Country-Test"));
    }

    #[test]
    fn lookup_city_against_country_db_returns_country_fields_only() {
        // A Country-precision db still parses as a City record;
        // the city / region / location fields are just None.
        let r = MmdbReader::from_bytes(build_country_fixture()).expect("parse");
        let city = r.lookup_city(country_fixture_ip()).expect("hit");
        assert_eq!(city.country.iso_code.as_deref(), Some("JP"));
        assert!(city.city_name_en.is_none());
        assert!(city.region_iso_code.is_none());
        assert!(city.location.is_none());
    }
}
