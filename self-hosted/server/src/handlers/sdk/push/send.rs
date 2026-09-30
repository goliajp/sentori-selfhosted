//! POST `/v1/push/send` — queue a push for delivery.
//!
//! Phase D step 4 queues the push to `push_sends` in `queued`
//! status. A background worker (out-of-scope for this commit;
//! Phase D step 5+ adds the dispatcher loop) drains the queue,
//! calls the vendor (APNs / FCM / WebPush / HCM / MiPush), and
//! writes `push_delivery_logs` rows + flips `push_sends.status`.

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::AssertSqlSafe;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendBody {
    /// Target by:
    /// - `tokenIds`: explicit device_token UUIDs
    /// - OR `nativeTokens`: list of provider tokens
    /// - OR `topic`: dispatch to every device subscribed to topic
    /// - OR `appUserId`: app-side user id (all devices)
    #[serde(default)]
    pub token_ids: Vec<Uuid>,
    /// The same thing under the name it should have had. `spToken`
    /// is what `register` returns and what a backend stores; both
    /// names are accepted until the old one is retired, because
    /// renaming a field out from under a live integration is how a
    /// working push stops working for a reason nobody can see.
    #[serde(default)]
    pub sp_tokens: Vec<Uuid>,
    #[serde(default)]
    pub native_tokens: Vec<String>,
    #[serde(default)]
    pub topic: Option<String>,
    /// The app's own id for a person, as passed to `sentori.user()`.
    ///
    /// Hashed here before it is compared, because the column holds a
    /// hash. Shorthand for `audience: { user: "..." }`.
    #[serde(default)]
    pub app_user_id: Option<String>,
    /// Attributes every target must have, as an object. Shorthand for
    /// an `audience` whose conditions are all equalities on traits.
    #[serde(default)]
    pub traits: Option<Value>,
    /// The audience as an expression. See `crate::audience`.
    #[serde(default)]
    pub audience: Option<Value>,
    /// Vendor payload (passed through verbatim to vendor adapter).
    pub payload: Value,
    /// Caller-supplied dedup key.
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub campaign_id: Option<String>,
    #[serde(default)]
    pub template_id: Option<String>,
}

/// How many devices one call may queue for.
///
/// A cap that silently truncated a broadcast would be worse than no
/// cap: the caller would believe they had reached everyone. It is
/// reported back as `capped`, and the preview endpoint answers how
/// large an audience is before anything is queued.
const MAX_TARGETS: i64 = 100_000;

/// How many explicit ids one call may name.
///
/// Each becomes its own placeholder, and a statement with a hundred
/// thousand of them is a parser problem rather than a query. Callers
/// with more than this are describing an audience, not a list.
const MAX_EXPLICIT: usize = 1_000;

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<SendBody>,
) -> (StatusCode, Json<Value>) {
    // A public token is in the customer's shipped app. Whoever pulls
    // it out could otherwise push arbitrary notifications to that
    // customer's users, from a channel those users trust.
    if let Err((code, body)) = crate::handlers::sdk::require_admin_token(&ctx) {
        return (code, body);
    }

    let selector = match Selector::build(&body) {
        Ok(s) => s,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "bad_target", "detail": msg })),
            );
        }
    };

    // One statement: the rows are chosen and queued in the same pass.
    //
    // It used to be two SELECTs and an INSERT per target — four
    // hundred round trips to notify two hundred devices, and there was
    // no shape of audience that did not make that worse.
    //
    // `ON CONFLICT DO NOTHING` is what makes `idempotencyKey` mean
    // anything. The unique index behind it is per device per key, so a
    // retry inserts nothing and `RETURNING` hands back only the rows
    // that were really queued — which is what `queued` should say. The
    // conflict target is left off deliberately: the only other unique
    // constraint here is the primary key, and naming the index couples
    // this statement to its exact shape.
    // One id for this call, stamped on every row it writes. It is what
    // the caller gets back and what the aggregate groups on — a
    // hundred and twenty-eight ids for one send was homework, not an
    // API.
    let batch_id = Uuid::now_v7();

    let sql = INSERT_TEMPLATE.replace("{selector}", &selector.sql);

    let mut q = sqlx::query(AssertSqlSafe(sql.clone()))
        .bind(ctx.project_id)
        .bind(&body.payload)
        .bind(body.idempotency_key.as_deref())
        .bind(body.campaign_id.as_deref())
        .bind(body.template_id.as_deref())
        .bind(batch_id)
        .bind(MAX_TARGETS);
    for b in &selector.binds {
        q = b.attach(q);
    }

    let rows = match q.fetch_all(&state.pool).await {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "push.send insert_failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            );
        }
    };

    // The rows are counted, not listed. `GET /v1/push/sends/{sendId}`
    // reports on the call and `/deliveries` walks the rows, so an
    // array of ids in this response was three megabytes of uuid for a
    // large send and homework for a small one.
    let queued = rows.len();

    info!(
        project_id = %ctx.project_id,
        queued,
        "push.send queued",
    );
    (
        StatusCode::ACCEPTED,
        Json(json!({
            // The id for this call. One poll of
            // `GET /v1/push/sends/{sendId}` answers "did it go out",
            // whatever the size of the audience.
            "sendId": batch_id.to_string(),
            "queued": queued,
            // True means there were more devices than this call would
            // take, so somebody did not get it. Never omitted, so a
            // caller can check for it without knowing it exists.
            "capped": i64::try_from(queued).unwrap_or(i64::MAX) >= MAX_TARGETS,
        })),
    )
}

/// The `WHERE` clause naming the devices, and its binds.
///
/// Every targeting mode compiles into this one thing, so the parts
/// that must be true of all of them — the project, the revocation
/// check, the cap — are written once and cannot be true of only some.
#[derive(Debug)]
pub struct Selector {
    pub sql: String,
    pub binds: Vec<crate::audience::Bind>,
}

impl Selector {
    /// Compile whatever the body named, or say why it named nothing.
    ///
    /// Modes given together are a union: `tokenIds` plus `topic` means
    /// both. An audience is one of the modes rather than a filter over
    /// the others — "these ids, but only the pro ones" is a question
    /// nobody has asked, and reading it that way silently would drop
    /// devices a caller listed explicitly.
    pub fn build(body: &SendBody) -> Result<Self, String> {
        let mut binds: Vec<crate::audience::Bind> = Vec::new();
        let mut parts: Vec<String> = Vec::new();

        // `spToken` is the name; `tokenIds` is the name it shipped
        // under. Both are accepted and mean the same column.
        let ids: Vec<&Uuid> = body.token_ids.iter().chain(body.sp_tokens.iter()).collect();
        if !ids.is_empty() {
            if ids.len() > MAX_EXPLICIT {
                return Err(format!(
                    "{} device ids in one call, which is more than the {MAX_EXPLICIT} \
                     allowed; describe them with `audience` instead",
                    ids.len()
                ));
            }
            let placeholders: Vec<String> = ids
                .iter()
                .map(|id| {
                    binds.push(crate::audience::Bind::Uuid(**id));
                    format!("${}", binds.len() + FIXED_BINDS)
                })
                .collect();
            parts.push(format!("dt.id IN ({})", placeholders.join(", ")));
        }

        if !body.native_tokens.is_empty() {
            if body.native_tokens.len() > MAX_EXPLICIT {
                return Err(format!(
                    "{} native tokens in one call, which is more than the \
                     {MAX_EXPLICIT} allowed",
                    body.native_tokens.len()
                ));
            }
            let placeholders: Vec<String> = body
                .native_tokens
                .iter()
                .map(|t| {
                    binds.push(crate::audience::Bind::Text(t.clone()));
                    format!("${}", binds.len() + FIXED_BINDS)
                })
                .collect();
            parts.push(format!("dt.native_token IN ({})", placeholders.join(", ")));
        }

        if let Some(topic) = body.topic.as_deref() {
            binds.push(crate::audience::Bind::Text(topic.to_string()));
            parts.push(format!(
                "EXISTS (SELECT 1 FROM device_topics tt \
                 WHERE tt.device_token_id = dt.id AND tt.topic = (${})::text)",
                binds.len() + FIXED_BINDS
            ));
        }

        if let Some(audience) = crate::audience::from_request(
            body.app_user_id.as_deref(),
            body.traits.as_ref(),
            body.audience.as_ref(),
        )? {
            let (frag, mut more) = audience.to_sql(binds.len() + FIXED_BINDS + 1);
            binds.append(&mut more);
            parts.push(frag);
        }

        if parts.is_empty() {
            // Without this the `WHERE` would be empty and the send
            // would go to the entire project — the one failure this
            // endpoint must not have.
            return Err("no target: give spTokens, nativeTokens, topic, appUserId, \
                        traits or audience"
                .to_string());
        }

        Ok(Selector {
            sql: parts.join(" OR "),
            binds,
        })
    }
}

/// The statement, with the selector left as a hole.
///
/// A `const` rather than an inline `format!` so a test can read the
/// placeholders back out of it. Adding `batch_id` as `$7` put it on
/// the same number as the selector's first bind, and the whole send
/// silently queued nothing — the same collision that had already sent
/// a fragment of a targeting condition out as a notification's
/// payload. A constant nobody can inspect is a constant that drifts.
// The conflict target is named, not left to "any unique index".
// `push_sends` carries two: the primary key, and the partial
// idempotency index. An untargeted `DO NOTHING` swallows both, so a
// primary-key collision — a uuid generator gone wrong, the thing we
// would most want to hear about — would be indistinguishable from
// the retry this clause exists to absorb.
//
// It also happens to be the form a backend can act on: SPG 7.38.4
// honours an untargeted `DO NOTHING` for a plain unique index and
// raises for a partial one. Naming the index is right on its own and
// portable as a side effect.
const INSERT_TEMPLATE: &str = "INSERT INTO push_sends \
     (id, project_id, token_id, provider, payload, status, \
      idempotency_key, campaign_id, template_id, batch_id) \
   SELECT gen_random_uuid(), $1, dt.id, dt.provider, $2, 'queued', $3, $4, $5, $6 \
   FROM ( \
     SELECT dt.id, dt.provider FROM device_tokens dt \
     WHERE dt.project_id = $1 AND dt.revoked_at IS NULL AND ({selector}) \
     ORDER BY dt.id LIMIT $7 \
   ) dt \
   ON CONFLICT (project_id, token_id, idempotency_key) \
     WHERE idempotency_key IS NOT NULL DO NOTHING \
   RETURNING id";

/// Placeholders the statement uses before the selector's own.
///
/// Checked against `INSERT_TEMPLATE` by a test rather than trusted:
/// this number and that statement have to move together, and the last
/// time they did not the endpoint queued nothing at all.
const FIXED_BINDS: usize = 7;

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> SendBody {
        SendBody {
            token_ids: Vec::new(),
            sp_tokens: Vec::new(),
            native_tokens: Vec::new(),
            topic: None,
            app_user_id: None,
            traits: None,
            audience: None,
            payload: json!({}),
            idempotency_key: None,
            campaign_id: None,
            template_id: None,
        }
    }

    /// The placeholders the selector emits are exactly the binds it
    /// produced, offset past the fixed ones.
    ///
    /// Every mode at once, because the numbering is shared state
    /// across them: the bug this catches is a mode that counts its own
    /// binds rather than the running total, which only shows up when
    /// something else came first.
    #[test]
    fn every_mode_at_once_numbers_its_placeholders_correctly() {
        let mut b = body();
        b.sp_tokens = vec![Uuid::nil(), Uuid::max()];
        b.native_tokens = vec!["a".into(), "b".into()];
        b.topic = Some("news".into());
        b.audience = Some(json!({ "all": [
            { "trait": "plan", "is": "pro" },
            { "device": "appVersion", "versionGte": "4.2" } ] }));

        let probe = Selector::build(&b);
        assert!(
            probe.is_ok(),
            "a body naming every mode did not compile: {probe:?}"
        );
        let Ok(sel) = probe else { return };
        let mut seen: Vec<usize> = sel
            .sql
            .split('$')
            .skip(1)
            .filter_map(|s| {
                s.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .ok()
            })
            .collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(
            seen,
            (FIXED_BINDS + 1..=FIXED_BINDS + sel.binds.len()).collect::<Vec<_>>(),
            "placeholders do not line up with binds: {}",
            sel.sql
        );
    }

    /// `FIXED_BINDS` is what the statement actually uses.
    ///
    /// The selector numbers its own placeholders from this, so if the
    /// statement grows one and this does not, the new column and the
    /// selector's first bind land on the same number. That happened —
    /// `batch_id` took `$7`, which was the audience's — and the
    /// endpoint queued nothing, silently, for every shape of target.
    #[test]
    fn the_fixed_placeholders_are_the_ones_the_statement_uses() {
        let highest = INSERT_TEMPLATE
            .split('$')
            .skip(1)
            .filter_map(|s| {
                s.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse::<usize>()
                    .ok()
            })
            .max()
            .unwrap_or(0);
        assert_eq!(
            highest,
            FIXED_BINDS,
            "the statement uses ${highest} but the selector is told to start at \
             ${}, so one of them writes over the other",
            FIXED_BINDS + 1
        );
    }

    /// A body that names nothing is refused rather than compiled into
    /// a clause that matches the whole project.
    #[test]
    fn a_body_with_no_target_is_refused() {
        let err = Selector::build(&body()).err();
        assert!(
            err.as_ref().is_some_and(|e| e.contains("no target")),
            "got {err:?}"
        );
    }

    /// The mode that used to answer 400 now answers with a clause.
    #[test]
    fn app_user_id_resolves_to_a_condition_on_the_identity_column() {
        let mut b = body();
        b.app_user_id = Some("usr_123".into());
        let probe = Selector::build(&b);
        assert!(probe.is_ok(), "appUserId did not compile: {probe:?}");
        let Ok(sel) = probe else { return };
        assert!(
            sel.sql.contains("dt.user_key"),
            "appUserId compiled to something that does not look at the identity \
             column: {}",
            sel.sql
        );
    }

    /// Explicit ids stay a list, not an audience.
    #[test]
    fn too_many_explicit_ids_are_refused_rather_than_truncated() {
        let mut b = body();
        b.sp_tokens = (0..=MAX_EXPLICIT).map(|_| Uuid::nil()).collect();
        let err = Selector::build(&b).err();
        assert!(
            err.as_ref().is_some_and(|e| e.contains("audience")),
            "got {err:?}"
        );
    }
}
