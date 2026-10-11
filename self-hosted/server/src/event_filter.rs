//! One event must satisfy every selected dimension, including all tags.
use axum::{Json, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Default, Deserialize)]
pub struct EventFilter {
    #[serde(alias = "appId")]
    pub app_id: Option<Uuid>,
    #[serde(alias = "appType")]
    pub platform: Option<String>,
    pub environment: Option<String>,
    pub release: Option<String>,
    pub context_key: Option<String>,
    pub context_value: Option<String>,
    /// JSON array of {source: context|tags, key, value}; values use text form.
    pub tags: Option<String>,
}

impl EventFilter {
    pub fn validated(&self) -> Result<Value, (StatusCode, Json<Value>)> {
        let invalid = |field| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid_filter", "field": field})),
            )
        };
        if self.platform.as_deref().is_some_and(|p| {
            !["ios", "android", "web", "weapp", "javascript", "unknown"].contains(&p)
        }) {
            return Err(invalid("platform"));
        }
        for (field, value) in [
            ("environment", &self.environment),
            ("release", &self.release),
        ] {
            if value.as_ref().is_some_and(|v| v.len() > 1000) {
                return Err(invalid(field));
            }
        }
        let mut tags: Vec<Value> = match &self.tags {
            Some(raw) if raw.len() <= 16_000 => {
                serde_json::from_str(raw).map_err(|_| invalid("tags"))?
            }
            Some(_) => return Err(invalid("tags")),
            None => vec![],
        };
        match (&self.context_key, &self.context_value) {
            (Some(key), Some(value)) => {
                tags.push(json!({"source":"context", "key":key, "value":value}));
            }
            (None, None) => (),
            _ => return Err(invalid("context_key")),
        }
        if tags.len() > 12
            || tags.iter().any(|tag| {
                !matches!(
                    tag.get("source").and_then(Value::as_str),
                    Some("context" | "tags")
                ) || !tag
                    .get("key")
                    .and_then(Value::as_str)
                    .is_some_and(|k| !k.is_empty() && k.len() <= 200)
                    || tag
                        .get("value")
                        .and_then(Value::as_str)
                        .is_none_or(|v| v.len() > 1000)
            })
        {
            return Err(invalid("tags"));
        }
        Ok(
            json!({"appId":self.app_id,"platform":self.platform,"environment":self.environment,"release":self.release,"tags":tags}),
        )
    }
}

/// Only an internal bind index is interpolated; all filter values stay bound.
pub fn predicate(bind: u8) -> String {
    let p = format!("${bind}::jsonb");
    format!(
        "({p}->>'appId' IS NULL OR e.app_id=({p}->>'appId')::uuid)
        AND ({p}->>'platform' IS NULL OR e.platform={p}->>'platform')
        AND ({p}->>'environment' IS NULL OR e.environment={p}->>'environment')
        AND ({p}->>'release' IS NULL OR e.release={p}->>'release')
        AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements({p}->'tags') tag
            WHERE (e.payload->(tag->>'source')->>(tag->>'key')) IS DISTINCT FROM tag->>'value')"
    )
}
