//! RFC 3339 timestamps for responses built with `serde_json::json!`.
//!
//! `#[serde(with = "time::serde::rfc3339")]` is a *field* attribute. It
//! cannot apply to a value interpolated into `json!`, because there is
//! no struct and therefore no field to annotate — and the default
//! `Serialize` for `OffsetDateTime` is a nine-element array:
//!
//! ```text
//! "created_at": [1970,1,0,0,0,0,0,0,0]
//! ```
//!
//! Which `new Date()` reads as Invalid Date. Before the dashboard moved
//! to `Intl.RelativeTimeFormat` that surfaced as a cosmetic "NaNy ago";
//! afterwards `Intl` threw on the non-finite number and took whole
//! pages down with it.
//!
//! `scripts/check-rfc3339.sh` catches the struct-field case and has
//! done since the 62-field drift in v1.7.3, but it scans for field
//! declarations, so twenty-six `json!` call sites sat in its blind
//! spot. The script now covers this shape too; these helpers are what
//! it points violations at.

use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// A timestamp as an RFC 3339 string, for interpolation into `json!`.
///
/// Formatting cannot fail for a value that came out of Postgres —
/// `Rfc3339` rejects only years outside 0000–9999, which `timestamptz`
/// cannot hold — but a panic in a list handler is a worse answer than a
/// null, so an unformattable instant is reported as absent.
pub fn rfc3339(ts: OffsetDateTime) -> Value {
    ts.format(&Rfc3339).map_or(Value::Null, Value::String)
}

/// The same, for a column that is genuinely nullable.
pub fn rfc3339_opt(ts: Option<OffsetDateTime>) -> Value {
    ts.map_or(Value::Null, rfc3339)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_as_an_rfc3339_string_not_an_array() {
        let v = rfc3339(OffsetDateTime::UNIX_EPOCH);
        assert_eq!(v, Value::String("1970-01-01T00:00:00Z".into()));
    }

    #[test]
    fn absent_timestamp_is_null_not_epoch() {
        // `new Date(null)` is the epoch, so a null that reaches the
        // dashboard as `0` would read as "56 years ago" rather than
        // "never". JSON null is what the client checks for.
        assert_eq!(rfc3339_opt(None), Value::Null);
    }

    /// The regression this module exists to prevent.
    #[test]
    fn bare_offsetdatetime_in_json_macro_would_be_an_array() {
        let raw = serde_json::json!({ "at": OffsetDateTime::UNIX_EPOCH });
        assert!(
            raw["at"].is_array(),
            "time's default Serialize changed; the json! helpers may no \
             longer be necessary — re-check before removing them"
        );
    }
}

#[cfg(test)]
mod field_attr_scope {
    use serde::Serialize;
    #[derive(Serialize)]
    struct M {
        #[serde(with = "time::serde::rfc3339")]
        added_at: time::OffsetDateTime,
    }
    /// The attribute applies when the *struct* is serialised…
    // A struct of one timestamp that will not serialise means serde
    // itself is broken; there is nothing for the test to report but that.
    #[allow(clippy::panic)]
    #[test]
    fn annotated_struct_is_rfc3339() {
        let m = M {
            added_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        let Ok(v) = serde_json::to_value(&m) else {
            panic!("a struct of one timestamp must serialise")
        };
        assert!(v["added_at"].is_string());
    }
    /// …and not when the field's value is lifted into json! on its own.
    #[test]
    fn same_field_via_json_macro_is_an_array() {
        let m = M {
            added_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        assert!(serde_json::json!({ "added_at": m.added_at })["added_at"].is_array());
    }
}
