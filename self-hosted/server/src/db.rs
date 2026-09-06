//! The one place that opens a database connection.
//!
//! Every session this process opens is pinned to `TimeZone = UTC`,
//! and every session is asked whether the pin took.
//!
//! Why it is pinned. `now()`, every `timestamptz` we render, every
//! `interval` arithmetic in the retention deletes and every
//! `extract(epoch FROM …)` in the workers is evaluated in the
//! *session's* time zone. That zone comes from the server's
//! `postgresql.conf` unless the client says otherwise, so the same
//! query against the same data gives different answers on two
//! deployments of the same image — and nothing in the result says
//! which zone produced it. 185 sites of ours depend on this and none
//! of them names a zone.
//!
//! Why it is checked rather than assumed. The pin travels in the
//! startup packet — sqlx names `TimeZone`, `DateStyle`,
//! `client_encoding` and `extra_float_digits` there on every
//! connection, and `options` carries ours — which is how libpq does
//! it and costs no round trip. An engine that accepts the startup
//! packet and discards its settings then answers every query in its
//! own zone, and nothing in any result says so. That is not
//! hypothetical: SPG 7.40.9 discards both channels (measured — it
//! reports `extra_float_digits = 1` while the driver asked for `2`).
//! So `after_connect` reads the zone back and refuses the connection
//! if it is not the one we asked for; a process that cannot pin the
//! zone does not get to compute timestamps.
//!
//! It reads it with `current_setting()` rather than `SHOW` because
//! `SHOW` is a utility statement and `current_setting` is a function
//! call, and this runs on the extended protocol where a function call
//! is the ordinary shape. (`SHOW` over the extended protocol is also
//! broken on SPG 7.40.9 — a `DataRow` with no `RowDescription` before
//! it — but that is not why this is written this way, and the corpus
//! keeps a case on it so it stays reported.)
//!
//! It runs on every physical connection, not once at boot, because a
//! pool opens connections hours later and a `SET` on one connection
//! says nothing about the next.

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use sqlx::{Connection, PgConnection};

/// The zone every session runs in. Not configurable: two deployments
/// disagreeing about it is the defect, and an operator who wants
/// local time can have it in the presentation layer.
const TIME_ZONE: &str = "UTC";

const READ_BACK: &str = "SELECT current_setting('TimeZone')";

fn options(url: &str) -> Result<PgConnectOptions, sqlx::Error> {
    Ok(url
        .parse::<PgConnectOptions>()?
        .options([("TimeZone", TIME_ZONE)]))
}

/// Read the zone back and refuse the connection if it is not ours.
async fn check(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    let zone: String = sqlx::query_scalar(READ_BACK).fetch_one(&mut *conn).await?;
    if zone == TIME_ZONE {
        return Ok(());
    }
    tracing::error!(
        got = %zone,
        want = TIME_ZONE,
        "the session TimeZone is not the one this connection asked for"
    );
    Err(sqlx::Error::Configuration(
        format!(
            "session TimeZone is `{zone}`, not `{TIME_ZONE}`. This connection asked for \
             it in the startup packet and the server did not take it, so every timestamp \
             this process computed would be in `{zone}` while the rest of the deployment \
             assumes `{TIME_ZONE}`."
        )
        .into(),
    ))
}

fn pinned(pool: PgPoolOptions) -> PgPoolOptions {
    pool.after_connect(|conn, _meta| Box::pin(check(conn)))
}

/// The same check, on one connection, before the pool is built.
///
/// A pool reports an `after_connect` failure as `pool timed out while
/// waiting for an open connection` — it retries until the acquire
/// timeout and the reason never reaches the operator. That message
/// names the wrong component, which is the shape of the compose
/// healthcheck bug that cost us an hour. So the first connection is
/// opened by hand, and it is the one that gets to explain itself.
async fn preflight(opts: &PgConnectOptions) -> Result<(), sqlx::Error> {
    let mut conn = PgConnection::connect_with(opts).await?;
    let verdict = check(&mut conn).await;
    let _ = conn.close().await;
    verdict
}

/// A pool with sqlx's default sizing, matching `PgPool::connect`.
pub async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    let opts = options(url)?;
    preflight(&opts).await?;
    pinned(PgPoolOptions::new()).connect_with(opts).await
}

/// A pool sized by the caller — one-shot subcommands want two, not ten.
pub async fn connect_with_max(url: &str, max_connections: u32) -> Result<PgPool, sqlx::Error> {
    let opts = options(url)?;
    preflight(&opts).await?;
    pinned(PgPoolOptions::new().max_connections(max_connections))
        .connect_with(opts)
        .await
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "a test asserting on a fixed URL")]
mod tests {
    use super::{TIME_ZONE, options};

    // The pin has to survive parsing a real URL, and it has to be the
    // zone the module documents rather than whatever was typed twice.
    #[test]
    fn the_url_still_parses_and_carries_the_zone() {
        let opts = options("postgres://u:p@localhost:5432/d").expect("a plain URL parses");
        let rendered = format!("{opts:?}");
        assert!(
            rendered.contains(TIME_ZONE),
            "the connect options do not mention {TIME_ZONE}: {rendered}"
        );
    }

    #[test]
    fn a_url_that_is_not_a_url_is_an_error_not_a_default() {
        assert!(options("not a database url").is_err());
    }
}
