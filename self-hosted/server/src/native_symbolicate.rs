//! Native-frame symbolication — proguard (Android) and DWARF (iOS),
//! plus the srcbundle source window.
//!
//! Both resolver crates shipped with tests and benchmarks and sat
//! dependency-declared with zero callers while insight-mobile's
//! 305 MB dSYM landed on the blob store unread. This is the caller.
//!
//! - **Android**: a JVM frame arrives as `function =
//!   "obf.Class.method"` + `line`. The release's R8 mapping
//!   deobfuscates it (with inline-chain expansion; we surface the
//!   outermost frame and mark the rest inline count).
//! - **iOS**: a frame that carries `addr` / `imageBase` /
//!   `imageUuid` (SDK ≥ 5.1) resolves through the dSYM slice whose
//!   uploaded name embeds the same debug id. Older SDKs send text
//!   symbols only — those stay as-is, honestly unsymbolicated.
//! - **srcbundle**: after either resolver produces file+line, a
//!   `srcbundle` artifact (source ZIP or legacy JSON, uploaded by the
//!   build) supplies the ±11-line reading window. No repository
//!   access, mirroring the JS sourcesContent path.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use sentori_dwarf_resolver::DwarfModule;
use sentori_proguard_resolver::ParsedMapping;

use crate::source_bundle::SrcBundle;

/// Rewrite the native frames of an error payload in place. Returns
/// how many frames were resolved.
pub async fn symbolicate_native(
    pool: &PgPool,
    attachments: &crate::blob_store::AttachmentStore,
    project_id: Uuid,
    release: &str,
    platform: &str,
    payload: &mut Value,
) -> usize {
    resolve(
        pool,
        attachments,
        project_id,
        release,
        platform,
        payload,
        false,
    )
    .await
    .unwrap_or(0)
}

pub async fn symbolicate_native_checked(
    pool: &PgPool,
    attachments: &crate::blob_store::AttachmentStore,
    project_id: Uuid,
    release: &str,
    platform: &str,
    payload: &mut Value,
) -> anyhow::Result<usize> {
    resolve(
        pool,
        attachments,
        project_id,
        release,
        platform,
        payload,
        true,
    )
    .await
}

async fn resolve(
    pool: &PgPool,
    attachments: &crate::blob_store::AttachmentStore,
    project_id: Uuid,
    release: &str,
    platform: &str,
    payload: &mut Value,
    strict: bool,
) -> anyhow::Result<usize> {
    if release.is_empty() {
        return Ok(0);
    }
    let Some(stack) = payload
        .get_mut("error")
        .and_then(|e| e.get_mut("stack"))
        .and_then(Value::as_array_mut)
    else {
        return Ok(0);
    };

    let srcbundle = artifact_bytes(
        pool,
        attachments,
        project_id,
        release,
        "srcbundle",
        None,
        strict,
    )
    .await?
    .and_then(|b| SrcBundle::parse(&b).ok());

    let mut resolved = std::collections::HashSet::new();
    match platform {
        "android" => {
            let mapping = artifact_bytes(
                pool,
                attachments,
                project_id,
                release,
                "proguard",
                None,
                strict,
            )
            .await?
            .and_then(|b| ParsedMapping::parse(b).ok());
            for (index, frame) in stack.iter_mut().enumerate() {
                if mapping
                    .as_ref()
                    .is_some_and(|mapping| rewrite_android(mapping, frame))
                {
                    resolved.insert(index);
                }
            }
        }
        "ios" => {
            // Per-frame dSYM slice lookup, cached per event by uuid.
            let mut modules: HashMap<String, Option<Arc<DwarfModule>>> = HashMap::new();
            for (index, frame) in stack.iter_mut().enumerate() {
                let Some(uuid) = frame
                    .get("imageUuid")
                    .and_then(Value::as_str)
                    .map(normalize_uuid)
                else {
                    continue;
                };
                if !modules.contains_key(&uuid) {
                    let m = artifact_bytes(
                        pool,
                        attachments,
                        project_id,
                        release,
                        "dsym",
                        Some(&uuid),
                        strict,
                    )
                    .await?
                    .and_then(|b| DwarfModule::from_bytes(b).ok().map(Arc::new));
                    modules.insert(uuid.clone(), m);
                }
                if let Some(Some(module)) = modules.get(&uuid)
                    && rewrite_ios(module, frame)
                {
                    resolved.insert(index);
                }
            }
        }
        _ => {}
    }
    // Already-readable native frames and source maps without embedded source
    // can use an uploaded source archive even without a native symbol file.
    for (index, frame) in stack.iter_mut().enumerate() {
        if fill_context(srcbundle.as_ref(), frame) {
            resolved.insert(index);
        }
    }
    Ok(resolved.len())
}

/// `"a.b.C.method"` → deobfuscated frame via the R8 mapping.
fn rewrite_android(mapping: &ParsedMapping, frame: &mut Value) -> bool {
    let Some(obj) = frame.as_object_mut() else {
        return false;
    };
    if obj.get("symbolicated").and_then(Value::as_bool) == Some(true) {
        return false;
    }
    let Some(function) = obj.get("function").and_then(Value::as_str) else {
        return false;
    };
    let Some((class, method)) = function.rsplit_once('.') else {
        return false;
    };
    let line = obj
        .get("line")
        .and_then(Value::as_u64)
        .and_then(|l| u32::try_from(l).ok())
        .unwrap_or(0);
    let Ok(frames) = mapping.resolve_method(class, method, line) else {
        return false;
    };
    let Some(top) = frames.first() else {
        return false;
    };
    obj.insert("minifiedFunction".into(), Value::from(function.to_owned()));
    obj.insert("function".into(), Value::from(top.full_method.clone()));
    if let Some(f) = &top.file {
        obj.insert("file".into(), Value::from(f.clone()));
    }
    if let Some(l) = top.line {
        obj.insert("line".into(), Value::from(l));
    }
    if frames.len() > 1 {
        obj.insert("inlineDepth".into(), Value::from(frames.len() - 1));
    }
    obj.insert("symbolicated".into(), Value::from(true));
    true
}

/// A frame with `addr`/`imageBase` resolves through its dSYM slice.
fn rewrite_ios(module: &DwarfModule, frame: &mut Value) -> bool {
    let Some(obj) = frame.as_object_mut() else {
        return false;
    };
    if obj.get("symbolicated").and_then(Value::as_bool) == Some(true) {
        return false;
    }
    let (Some(addr), Some(base)) = (
        obj.get("addr").and_then(Value::as_u64),
        obj.get("imageBase").and_then(Value::as_u64),
    ) else {
        return false;
    };
    let Some(offset) = addr.checked_sub(base) else {
        return false;
    };
    let Ok(frames) = module.resolve(offset) else {
        return false;
    };
    // Innermost-last convention: the lexical function at the PC is
    // the most useful headline; inline parents add depth count.
    let Some(top) = frames.last() else {
        return false;
    };
    if let Some(f) = &top.function {
        obj.insert("function".into(), Value::from(f.clone()));
    }
    if let Some(f) = &top.file {
        obj.insert("file".into(), Value::from(f.clone()));
    }
    if let Some(l) = top.line {
        obj.insert("line".into(), Value::from(l));
    }
    if frames.len() > 1 {
        obj.insert("inlineDepth".into(), Value::from(frames.len() - 1));
    }
    obj.insert("symbolicated".into(), Value::from(true));
    true
}

/// Fill the reading window from the srcbundle for a frame that has
/// file+line and no context yet.
fn fill_context(bundle: Option<&SrcBundle>, frame: &mut Value) -> bool {
    let Some(bundle) = bundle else { return false };
    let Some(obj) = frame.as_object_mut() else {
        return false;
    };
    if obj.get("contextLine").is_some_and(|value| !value.is_null()) {
        return false;
    }
    let (Some(file), Some(line)) = (
        obj.get("file").and_then(Value::as_str).map(str::to_owned),
        obj.get("line")
            .and_then(Value::as_u64)
            .and_then(|l| u32::try_from(l).ok()),
    ) else {
        return false;
    };
    let Some((pre, at, post)) = bundle.window(&file, line) else {
        return false;
    };
    obj.insert(
        "preContext".into(),
        Value::from(pre.into_iter().map(Value::from).collect::<Vec<_>>()),
    );
    obj.insert("contextLine".into(), Value::from(at));
    obj.insert(
        "postContext".into(),
        Value::from(post.into_iter().map(Value::from).collect::<Vec<_>>()),
    );
    true
}

pub(crate) fn normalize_uuid(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_uppercase()
}

/// Latest artifact of `kind` for the release; for dSYMs,
/// `name_contains` narrows to the slice whose uploaded name embeds
/// the frame's debug id.
async fn artifact_bytes(
    pool: &PgPool,
    attachments: &crate::blob_store::AttachmentStore,
    project_id: Uuid,
    release: &str,
    kind: &str,
    name_uuid: Option<&str>,
    strict: bool,
) -> anyhow::Result<Option<Vec<u8>>> {
    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT a.name, a.content_hash \
         FROM release_artifacts a \
         JOIN releases r ON r.id = a.release_id \
         WHERE r.project_id = $1 AND r.name = $2 AND a.kind = $3 \
         ORDER BY a.created_at DESC",
    )
    .bind(project_id)
    .bind(release)
    .bind(kind)
    .fetch_all(pool)
    .await;
    let rows = match rows {
        Ok(rows) => rows,
        Err(error) if strict => return Err(error.into()),
        Err(_) => return Ok(None),
    };
    let hash_hex = rows.iter().find_map(|r| {
        let name: String = r.get("name");
        match name_uuid {
            Some(u) => normalize_uuid(&name)
                .contains(u)
                .then(|| r.get::<String, _>("content_hash")),
            None => Some(r.get::<String, _>("content_hash")),
        }
    });
    let Some(hash_hex) = hash_hex else {
        return Ok(None);
    };
    let hash = match hash_hex.parse() {
        Ok(hash) => hash,
        Err(_) if strict => anyhow::bail!("invalid artifact hash"),
        Err(_) => return Ok(None),
    };
    match attachments.get(&hash).await {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if strict => Err(error.into()),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn android_frame_resolves_through_a_mapping() -> Result<(), Box<dyn std::error::Error>> {
        // Minimal R8 mapping: obfuscated a.a → com.example.Pay.charge
        let mapping = ParsedMapping::parse(
            b"com.example.Pay -> a.a:\n    1:10:void charge(int):20:29 -> b\n".to_vec(),
        )?;
        let mut frame = json!({ "function": "a.a.b", "line": 3, "inApp": true });
        assert!(rewrite_android(&mapping, &mut frame));
        let func = frame["function"].as_str().ok_or("fn missing")?;
        assert!(func.contains("com.example.Pay"));
        assert_eq!(frame["symbolicated"], true);
        assert_eq!(frame["minifiedFunction"], "a.a.b");
        Ok(())
    }

    #[test]
    fn srcbundle_window_fills_context() -> Result<(), Box<dyn std::error::Error>> {
        let src = (1..=30)
            .map(|n| format!("kt line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let bundle = SrcBundle::parse(json!({ "app/src/Pay.kt": src }).to_string().as_bytes())?;
        let mut frame = json!({ "file": "Pay.kt", "line": 15 });
        fill_context(Some(&bundle), &mut frame);
        assert_eq!(frame["contextLine"], "kt line 15");
        assert_eq!(
            frame["preContext"].as_array().ok_or("pre missing")?.len(),
            11
        );
        Ok(())
    }

    #[test]
    fn uuid_normalization_matches_cli_slice_names() {
        assert_eq!(
            normalize_uuid("E63A748C-3F0E-302D-95EC-8DA5B55C97D9"),
            normalize_uuid("e63a748c3f0e302d95ec8da5b55c97d9"),
        );
        assert!(
            normalize_uuid("Insight.app-arm64-E63A748C-3F0E-302D-95EC-8DA5B55C97D9")
                .contains(&normalize_uuid("E63A748C-3F0E-302D-95EC-8DA5B55C97D9"))
        );
    }
}
