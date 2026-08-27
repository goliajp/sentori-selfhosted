# Third-party data attribution

## DB-IP Lite IP-to-Country

The Sentori server image bundles the **DB-IP Lite IP-to-Country**
database for GeoIP enrichment of ingested events. This database is
made available by DB-IP under the **Creative Commons Attribution
4.0 International License** (CC-BY 4.0).

- Source: <https://db-ip.com/db/ip-to-country-lite>
- License: <https://creativecommons.org/licenses/by/4.0/>
- Attribution text required on user-facing material that surfaces the
  data:

    > IP geolocation by DB-IP

The current bundled version is selected at docker image build time via
the `DBIP_VERSION` build arg (default tracks the most recent
month-tagged release).
