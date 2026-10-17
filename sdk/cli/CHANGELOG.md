# @goliapkg/sentori-cli

## 1.8.0

### Minor Changes

- Inspect Web performance and manage isolated browser checks with idempotent commands, strict deployment results and explicit token-project verification for uploads.

## 1.7.0

### Minor Changes

- Carry stable browser app identity in events and sessions. Inspect the token-selected project and its apps, and filter issue aggregates by app, platform, environment and release. Accept SENTORI_API_TOKEN for inspection and direct uploads.

## 1.6.0

### Minor Changes

- Add the sentori command and retain sentori-cli as an alias. Configure, pack source ZIPs from directories or exact Git tags, upload all selected debug artifacts and verify their exact stored bytes. Add --version and doctor; negotiate server artifact capabilities before source ZIP uploads. Document CLI/server compatibility, installation, platform recipes and strict CI gates.

## 0.6.0

### Minor Changes

- [`4022db4`](https://github.com/goliajp/sentori-selfhosted/commit/4022db4fcff42568412158948c513851d099e0e0) Thanks [@doracawl](https://github.com/doracawl)! - v2.12 — HCM + MiPush server providers + framework push hooks + sentori-cli push commands. Closes the v2.7→v2.12 Push rollout.

  Sixth and final phase of the multi-version Push capability. v2.7
  shipped the server foundation, v2.8-v2.11 lit Web Push / RN iOS /
  RN Android / Expo plugin + dashboard. v2.12 wraps the series with
  the Chinese-market provider impls + framework wrappers + CLI.

  **Server — `hcm.rs` (Huawei HMS Push Kit)**

  - OAuth `client_credentials` token mint via
    `oauth-login.cloud.huawei.com/oauth2/v3/token`; cached per app_id
    for `expires_in - 60s`.
  - POST `push-api.cloud.huawei.com/v1/<app_id>/messages:send` with
    `Bearer <access_token>`. Message envelope packs `token: [reg_id]`,
    `notification: { title, body }`, and HMS's required-string-encoded
    `data` field. Android urgency mapped from NativeMessage priority.
  - Outcome classifier: 80000000=Sent / 80200001 + 80200003 =
    PermanentlyInvalidToken / 80300008 = MessageTooBig / 80100003 =
    Transient / 401 / 429 / 5xx all handled per HMS docs.
  - 6 unit tests cover the classifier matrix + HMS message build.

  **Server — `mipush.rs` (Xiaomi MiPush)**

  - `Authorization: key=<AppSecret>` auth (no OAuth dance — much
    simpler than HCM).
  - Form-encoded POST to `api.xmpush.xiaomi.com/v3/message/regid`
    (CN) or `.global.xmpush.xiaomi.com` (global). Body fields:
    registration_id / payload / restricted_package_name /
    pass_through / notify_type / title / description / time_to_live.
  - Outcome classifier: 200+code=0+result=ok→Sent / 22000 =
    PermanentlyInvalidToken / 22020 = Transient / 22021 = MessageTooBig
    / 401 / 429 / 5xx.
  - 5 unit tests cover the classifier matrix.

  **Framework hooks**

  - `@goliapkg/sentori-react` — `useSentoriPush({ vapidPublicKey })`
    hook returning `{ ipt, permission, error, register, unregister }`.
    Reactive over the cached ipt + Notification.permission. Hosts
    bind UI to the returned state idiomatically.
  - `@goliapkg/sentori-vue` / `@goliapkg/sentori-svelte` /
    `@goliapkg/sentori-solid` — passthrough re-exports of
    `registerWeb` / `unregisterWeb` / `readCachedIpt` / push types
    from `@goliapkg/sentori-javascript` + `@goliapkg/sentori-core`.
    Each framework's idiomatic state wrapper (composable / store /
    signal) is host-shaped enough that we ship the primitives + types
    and stop short of bundling one for everyone.

  **`@goliapkg/sentori-cli` push commands**

  Five subcommands wrapping the v2.7 admin REST + ingest endpoints:

  - `sentori push send -p <proj> --to <ipt> --title --body [--priority high|normal] [--ttl <s>] [--data @json] [--idempotency-key <s>]`
  - `sentori push receipt -p <proj> <send-id>`
  - `sentori push creds list -p <proj>`
  - `sentori push creds set <provider> -p <proj> --config @config.json --secret @secret.json`
  - `sentori push creds delete <provider> -p <proj>`

  Same admin Bearer auth as the existing `issue` subcommand;
  SENTORI_ADMIN_TOKEN env var + SENTORI_PROJECT_ID env var picked up
  automatically.

  `@file.json` shorthand for `--config` / `--data` / `--secret` reads
  the file from disk + parses as JSON. Plain JSON literals also
  accepted for one-off ad-hoc sends.

  **Series close**

  v2.7→v2.12 ships an end-to-end push capability: 5 providers (APNs

  - FCM + Web Push + HCM + MiPush) on the server side, opt-in browser
  - RN-iOS + RN-Android SDKs, Expo config plugin auto-injecting the
    host setup, dashboard credential management, and CLI for operator
    workflows. 167 server lib tests, 186 RN tests, full SDK matrix
    clean. Wire shape held stable across all six versions — customers
    who integrated against the v2.7 raw REST stay working with no
    changes through v2.12.

## 0.6.0

### Minor Changes

- [`4022db4`](https://github.com/goliajp/sentori-selfhosted/commit/4022db4fcff42568412158948c513851d099e0e0) Thanks [@doracawl](https://github.com/doracawl)! - v2.12 — HCM + MiPush server providers + framework push hooks + sentori-cli push commands. Closes the v2.7→v2.12 Push rollout.

  Sixth and final phase of the multi-version Push capability. v2.7
  shipped the server foundation, v2.8-v2.11 lit Web Push / RN iOS /
  RN Android / Expo plugin + dashboard. v2.12 wraps the series with
  the Chinese-market provider impls + framework wrappers + CLI.

  **Server — `hcm.rs` (Huawei HMS Push Kit)**

  - OAuth `client_credentials` token mint via
    `oauth-login.cloud.huawei.com/oauth2/v3/token`; cached per app_id
    for `expires_in - 60s`.
  - POST `push-api.cloud.huawei.com/v1/<app_id>/messages:send` with
    `Bearer <access_token>`. Message envelope packs `token: [reg_id]`,
    `notification: { title, body }`, and HMS's required-string-encoded
    `data` field. Android urgency mapped from NativeMessage priority.
  - Outcome classifier: 80000000=Sent / 80200001 + 80200003 =
    PermanentlyInvalidToken / 80300008 = MessageTooBig / 80100003 =
    Transient / 401 / 429 / 5xx all handled per HMS docs.
  - 6 unit tests cover the classifier matrix + HMS message build.

  **Server — `mipush.rs` (Xiaomi MiPush)**

  - `Authorization: key=<AppSecret>` auth (no OAuth dance — much
    simpler than HCM).
  - Form-encoded POST to `api.xmpush.xiaomi.com/v3/message/regid`
    (CN) or `.global.xmpush.xiaomi.com` (global). Body fields:
    registration_id / payload / restricted_package_name /
    pass_through / notify_type / title / description / time_to_live.
  - Outcome classifier: 200+code=0+result=ok→Sent / 22000 =
    PermanentlyInvalidToken / 22020 = Transient / 22021 = MessageTooBig
    / 401 / 429 / 5xx.
  - 5 unit tests cover the classifier matrix.

  **Framework hooks**

  - `@goliapkg/sentori-react` — `useSentoriPush({ vapidPublicKey })`
    hook returning `{ ipt, permission, error, register, unregister }`.
    Reactive over the cached ipt + Notification.permission. Hosts
    bind UI to the returned state idiomatically.
  - `@goliapkg/sentori-vue` / `@goliapkg/sentori-svelte` /
    `@goliapkg/sentori-solid` — passthrough re-exports of
    `registerWeb` / `unregisterWeb` / `readCachedIpt` / push types
    from `@goliapkg/sentori-javascript` + `@goliapkg/sentori-core`.
    Each framework's idiomatic state wrapper (composable / store /
    signal) is host-shaped enough that we ship the primitives + types
    and stop short of bundling one for everyone.

  **`@goliapkg/sentori-cli` push commands**

  Five subcommands wrapping the v2.7 admin REST + ingest endpoints:

  - `sentori push send -p <proj> --to <ipt> --title --body [--priority high|normal] [--ttl <s>] [--data @json] [--idempotency-key <s>]`
  - `sentori push receipt -p <proj> <send-id>`
  - `sentori push creds list -p <proj>`
  - `sentori push creds set <provider> -p <proj> --config @config.json --secret @secret.json`
  - `sentori push creds delete <provider> -p <proj>`

  Same admin Bearer auth as the existing `issue` subcommand;
  SENTORI_ADMIN_TOKEN env var + SENTORI_PROJECT_ID env var picked up
  automatically.

  `@file.json` shorthand for `--config` / `--data` / `--secret` reads
  the file from disk + parses as JSON. Plain JSON literals also
  accepted for one-off ad-hoc sends.

  **Series close**

  v2.7→v2.12 ships an end-to-end push capability: 5 providers (APNs

  - FCM + Web Push + HCM + MiPush) on the server side, opt-in browser
  - RN-iOS + RN-Android SDKs, Expo config plugin auto-injecting the
    host setup, dashboard credential management, and CLI for operator
    workflows. 167 server lib tests, 186 RN tests, full SDK matrix
    clean. Wire shape held stable across all six versions — customers
    who integrated against the v2.7 raw REST stay working with no
    changes through v2.12.

## 0.6.0

### Minor Changes

- [`4022db4`](https://github.com/goliajp/sentori-selfhosted/commit/4022db4fcff42568412158948c513851d099e0e0) Thanks [@doracawl](https://github.com/doracawl)! - v2.12 — HCM + MiPush server providers + framework push hooks + sentori-cli push commands. Closes the v2.7→v2.12 Push rollout.

  Sixth and final phase of the multi-version Push capability. v2.7
  shipped the server foundation, v2.8-v2.11 lit Web Push / RN iOS /
  RN Android / Expo plugin + dashboard. v2.12 wraps the series with
  the Chinese-market provider impls + framework wrappers + CLI.

  **Server — `hcm.rs` (Huawei HMS Push Kit)**

  - OAuth `client_credentials` token mint via
    `oauth-login.cloud.huawei.com/oauth2/v3/token`; cached per app_id
    for `expires_in - 60s`.
  - POST `push-api.cloud.huawei.com/v1/<app_id>/messages:send` with
    `Bearer <access_token>`. Message envelope packs `token: [reg_id]`,
    `notification: { title, body }`, and HMS's required-string-encoded
    `data` field. Android urgency mapped from NativeMessage priority.
  - Outcome classifier: 80000000=Sent / 80200001 + 80200003 =
    PermanentlyInvalidToken / 80300008 = MessageTooBig / 80100003 =
    Transient / 401 / 429 / 5xx all handled per HMS docs.
  - 6 unit tests cover the classifier matrix + HMS message build.

  **Server — `mipush.rs` (Xiaomi MiPush)**

  - `Authorization: key=<AppSecret>` auth (no OAuth dance — much
    simpler than HCM).
  - Form-encoded POST to `api.xmpush.xiaomi.com/v3/message/regid`
    (CN) or `.global.xmpush.xiaomi.com` (global). Body fields:
    registration_id / payload / restricted_package_name /
    pass_through / notify_type / title / description / time_to_live.
  - Outcome classifier: 200+code=0+result=ok→Sent / 22000 =
    PermanentlyInvalidToken / 22020 = Transient / 22021 = MessageTooBig
    / 401 / 429 / 5xx.
  - 5 unit tests cover the classifier matrix.

  **Framework hooks**

  - `@goliapkg/sentori-react` — `useSentoriPush({ vapidPublicKey })`
    hook returning `{ ipt, permission, error, register, unregister }`.
    Reactive over the cached ipt + Notification.permission. Hosts
    bind UI to the returned state idiomatically.
  - `@goliapkg/sentori-vue` / `@goliapkg/sentori-svelte` /
    `@goliapkg/sentori-solid` — passthrough re-exports of
    `registerWeb` / `unregisterWeb` / `readCachedIpt` / push types
    from `@goliapkg/sentori-javascript` + `@goliapkg/sentori-core`.
    Each framework's idiomatic state wrapper (composable / store /
    signal) is host-shaped enough that we ship the primitives + types
    and stop short of bundling one for everyone.

  **`@goliapkg/sentori-cli` push commands**

  Five subcommands wrapping the v2.7 admin REST + ingest endpoints:

  - `sentori push send -p <proj> --to <ipt> --title --body [--priority high|normal] [--ttl <s>] [--data @json] [--idempotency-key <s>]`
  - `sentori push receipt -p <proj> <send-id>`
  - `sentori push creds list -p <proj>`
  - `sentori push creds set <provider> -p <proj> --config @config.json --secret @secret.json`
  - `sentori push creds delete <provider> -p <proj>`

  Same admin Bearer auth as the existing `issue` subcommand;
  SENTORI_ADMIN_TOKEN env var + SENTORI_PROJECT_ID env var picked up
  automatically.

  `@file.json` shorthand for `--config` / `--data` / `--secret` reads
  the file from disk + parses as JSON. Plain JSON literals also
  accepted for one-off ad-hoc sends.

  **Series close**

  v2.7→v2.12 ships an end-to-end push capability: 5 providers (APNs

  - FCM + Web Push + HCM + MiPush) on the server side, opt-in browser
  - RN-iOS + RN-Android SDKs, Expo config plugin auto-injecting the
    host setup, dashboard credential management, and CLI for operator
    workflows. 167 server lib tests, 186 RN tests, full SDK matrix
    clean. Wire shape held stable across all six versions — customers
    who integrated against the v2.7 raw REST stay working with no
    changes through v2.12.
