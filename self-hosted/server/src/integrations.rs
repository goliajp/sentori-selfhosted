//! Outbound adapters. Secrets are sealed with an identity binding and never enter logs.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

pub const EVENTS: &[&str] = &[
    "issue.new",
    "issue.regressed",
    "task.assigned",
    "task.updated",
    "monitor.down",
    "monitor.recovered",
];
#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub repository: String,
    #[serde(default)]
    pub recipient: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub project_key: String,
    #[serde(default)]
    pub issue_type_id: String,
}
fn slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn email(value: &str) -> bool {
    value.len() <= 254
        && value
            .split_once('@')
            .is_some_and(|(user, host)| !user.is_empty() && host.contains('.'))
        && value
            .bytes()
            .all(|b| b.is_ascii_graphic() && !b"<>,;\"\\".contains(&b))
}
impl Settings {
    pub fn merge_secrets(&mut self, previous: &Self) {
        if self.endpoint.is_empty() {
            self.endpoint.clone_from(&previous.endpoint);
        }
        if self.token.is_empty() && self.endpoint == previous.endpoint {
            self.token.clone_from(&previous.token);
        }
    }
    pub fn validate(&self, kind: &str) -> Result<(), &'static str> {
        if self.token.contains(['\r', '\n']) {
            return Err("invalid_credential");
        }
        if [
            &self.endpoint,
            &self.token,
            &self.repository,
            &self.recipient,
            &self.email,
            &self.project_key,
            &self.issue_type_id,
        ]
        .iter()
        .any(|v| v.len() > 4096)
        {
            return Err("invalid_integration");
        }
        match kind {
            "email" if email(&self.recipient) => Ok(()),
            "github"
                if self
                    .repository
                    .split_once('/')
                    .is_some_and(|(a, b)| slug(a) && slug(b))
                    && !self.token.is_empty() =>
            {
                Ok(())
            }
            "webhook" | "jira" => {
                let url = crate::public_http::parse(&self.endpoint)?;
                if url.scheme() != "https" || self.token.len() < 16 {
                    return Err("https_and_secret_required");
                }
                if kind == "jira"
                    && (url.path() != "/"
                        || url.query().is_some()
                        || !email(&self.email)
                        || !slug(&self.project_key)
                        || self.issue_type_id.is_empty()
                        || self.issue_type_id.len() > 20
                        || !self.issue_type_id.bytes().all(|b| b.is_ascii_digit()))
                {
                    return Err("invalid_jira_settings");
                }
                Ok(())
            }
            _ => Err("invalid_integration"),
        }
    }
    pub fn summary(&self, kind: &str) -> Value {
        match kind {
            "email" => json!({"recipient":self.recipient}),
            "github" => json!({"repository":self.repository}),
            "jira" => {
                json!({"endpoint":self.endpoint,"email":self.email,"projectKey":self.project_key,"issueTypeId":self.issue_type_id})
            }
            _ => {
                json!({"endpointOrigin":reqwest::Url::parse(&self.endpoint).ok().map(|u|u.origin().ascii_serialization())})
            }
        }
    }
}
pub fn seal(
    vault: &sentori_secrets_vault::Vault,
    project: Uuid,
    kind: &str,
    settings: &Settings,
) -> Result<Vec<u8>, &'static str> {
    let value = serde_json::to_vec(&json!({"project":project,"kind":kind,"settings":settings}))
        .map_err(|_| "invalid_integration")?;
    vault.seal(&value).map_err(|_| "encryption_failed")
}
pub fn open(
    vault: &sentori_secrets_vault::Vault,
    project: Uuid,
    kind: &str,
    blob: &[u8],
) -> Result<Settings, &'static str> {
    let bytes = vault.open(blob).map_err(|_| "decryption_failed")?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "decryption_failed")?;
    if value["project"].as_str() != Some(project.to_string().as_str())
        || value["kind"].as_str() != Some(kind)
    {
        return Err("credential_scope_mismatch");
    }
    serde_json::from_value(value["settings"].clone()).map_err(|_| "decryption_failed")
}

pub struct Prepared {
    pub endpoint: String,
    pub headers: reqwest::header::HeaderMap,
    pub body: Vec<u8>,
}
pub fn prepare(
    kind: &str,
    settings: &Settings,
    delivery: Uuid,
    payload: &Value,
    base_url: &str,
) -> Result<Prepared, &'static str> {
    use base64::Engine;
    use hmac::{Hmac, KeyInit, Mac};
    let title = payload["title"].as_str().unwrap_or("Sentori notification");
    let text = payload["body"].as_str().unwrap_or("");
    let route = payload["route"].as_str().unwrap_or("/");
    let body = format!("{text}\n\n{base_url}{route}\n\nSentori delivery: {delivery}");
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "content-type",
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        "x-sentori-delivery",
        delivery.to_string().parse().map_err(|_| "invalid_header")?,
    );
    let (endpoint, wire) = match kind {
        "github" => {
            headers.insert(
                "accept",
                reqwest::header::HeaderValue::from_static("application/vnd.github+json"),
            );
            headers.insert(
                "x-github-api-version",
                reqwest::header::HeaderValue::from_static("2026-03-10"),
            );
            let mut auth: reqwest::header::HeaderValue = format!("Bearer {}", settings.token)
                .parse()
                .map_err(|_| "invalid_credential")?;
            auth.set_sensitive(true);
            headers.insert("authorization", auth);
            (
                format!(
                    "https://api.github.com/repos/{}/issues",
                    settings.repository
                ),
                json!({"title":title,"body":body}),
            )
        }
        "jira" => {
            let mut auth: reqwest::header::HeaderValue = format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD
                    .encode(format!("{}:{}", settings.email, settings.token))
            )
            .parse()
            .map_err(|_| "invalid_credential")?;
            auth.set_sensitive(true);
            headers.insert("authorization", auth);
            (
                format!(
                    "{}/rest/api/3/issue",
                    settings.endpoint.trim_end_matches('/')
                ),
                json!({"fields":{"summary":title,"project":{"key":settings.project_key},"issuetype":{"id":settings.issue_type_id},"description":{"type":"doc","version":1,"content":[{"type":"paragraph","content":[{"type":"text","text":body}]}]}}}),
            )
        }
        "webhook" | "legacy_webhook" => (
            settings.endpoint.clone(),
            json!({"id":delivery,"event":payload["event"],"subject":title,"body":body,"metadata":payload}),
        ),
        _ => return Err("invalid_integration"),
    };
    let body = serde_json::to_vec(&wire).map_err(|_| "invalid_payload")?;
    if kind == "webhook" {
        let timestamp = time::OffsetDateTime::now_utc().unix_timestamp().to_string();
        let mut mac = Hmac::<sha2::Sha256>::new_from_slice(settings.token.as_bytes())
            .map_err(|_| "invalid_credential")?;
        mac.update(timestamp.as_bytes());
        mac.update(b".");
        mac.update(&body);
        let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
        headers.insert(
            "x-sentori-timestamp",
            timestamp.parse().map_err(|_| "invalid_header")?,
        );
        headers.insert(
            "x-sentori-signature",
            signature.parse().map_err(|_| "invalid_header")?,
        );
    }
    Ok(Prepared {
        endpoint,
        headers,
        body,
    })
}

/// Build external links from validated identifiers, never arbitrary response URLs.
pub fn external_url(kind: &str, settings: &Settings, body: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(body).ok()?;
    match kind {
        "github" => value["number"]
            .as_u64()
            .filter(|n| *n > 0)
            .map(|number| format!("https://github.com/{}/issues/{number}", settings.repository)),
        "jira" => value["key"]
            .as_str()
            .filter(|key| slug(key))
            .map(|key| format!("{}/browse/{key}", settings.endpoint.trim_end_matches('/'))),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn scopes_and_redacts_webhook_secrets() {
        let vault = sentori_secrets_vault::Vault::new(
            sentori_secrets_vault::MasterKey::from_bytes([42; 32]),
            sentori_secrets_vault::KeyId::new("test").unwrap(),
        );
        let project = Uuid::now_v7();
        let settings = Settings {
            endpoint: "https://hooks.example.com/private-secret".into(),
            token: "secret-signing-key".into(),
            ..Settings::default()
        };
        let blob = seal(&vault, project, "webhook", &settings).unwrap();
        assert!(open(&vault, Uuid::now_v7(), "webhook", &blob).is_err());
        assert!(open(&vault, project, "jira", &blob).is_err());
        assert_eq!(
            open(&vault, project, "webhook", &blob).unwrap().token,
            settings.token
        );
        assert!(
            !settings
                .summary("webhook")
                .to_string()
                .contains("private-secret")
        );
        assert!(settings.validate("webhook").is_ok());
    }
    #[test]
    fn payloads_use_native_protocols_and_safe_links() {
        let payload = json!({"event":"task.export","title":"Fix checkout","body":"Evidence","route":"/tasks"});
        let github = Settings {
            repository: "team/repo".into(),
            token: "pat-token".into(),
            ..Settings::default()
        };
        let prepared = prepare(
            "github",
            &github,
            Uuid::now_v7(),
            &payload,
            "https://sentori.example.com",
        )
        .unwrap();
        assert_eq!(
            prepared.endpoint,
            "https://api.github.com/repos/team/repo/issues"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&prepared.body).unwrap()["title"],
            "Fix checkout"
        );
        assert_eq!(
            external_url(
                "github",
                &github,
                br#"{"number":42,"html_url":"javascript:alert(1)"}"#
            )
            .unwrap(),
            "https://github.com/team/repo/issues/42"
        );
        let jira = Settings {
            endpoint: "https://team.atlassian.net/".into(),
            email: "bot@example.com".into(),
            token: "token-secret-123456".into(),
            project_key: "ENG".into(),
            issue_type_id: "10001".into(),
            ..Settings::default()
        };
        assert!(jira.validate("jira").is_ok());
        let prepared = prepare(
            "jira",
            &jira,
            Uuid::now_v7(),
            &payload,
            "https://sentori.example.com",
        )
        .unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&prepared.body).unwrap()["fields"]["description"]["type"],
            "doc"
        );
        assert!(external_url("jira", &jira, br#"{"key":"../admin"}"#).is_none());
    }
}
