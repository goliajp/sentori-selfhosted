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
//!   `srcbundle` artifact (JSON path→content, uploaded by the
//!   build) supplies the ±5-line reading window. No repository
//!   access, mirroring the JS sourcesContent path.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use sentori_dwarf_resolver::DwarfModule;
use sentori_proguard_resolver::ParsedMapping;

const CONTEXT_LINES: usize = 11;
const MAX_CONTEXT_LINE_CHARS: usize = 300;

/// Parsed srcbundle: project-relative path → file content.
pub struct SrcBundle {
    files: HashMap<String, String>,
}

impl SrcBundle {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let v: Value = serde_json::from_slice(bytes).ok()?;
        let obj = v.as_object()?;
        let mut files = HashMap::new();
        for (k, val) in obj {
            if let Some(s) = val.as_str() {
                files.insert(k.clone(), s.to_owned());
            }
        }
        Some(Self { files })
    }

    /// Look a compiler-recorded path up leniently: exact, then by
    /// suffix — DWARF records build-host absolute paths, the bundle
    /// carries project-relative ones.
    fn lookup(&self, path: &str) -> Option<&str> {
        if let Some(c) = self.files.get(path) {
            return Some(c);
        }
        self.files
            .iter()
            .find(|(k, _)| path.ends_with(k.as_str()) || k.ends_with(path))
            .map(|(_, v)| v.as_str())
    }

    fn window(&self, path: &str, line: u32) -> Option<(Vec<String>, String, Vec<String>)> {
        let content = self.lookup(path)?;
        let rows: Vec<&str> = content.lines().collect();
        let line0 = usize::try_from(line).ok()?.checked_sub(1)?;
        if line0 >= rows.len() {
            return None;
        }
        let start = line0.saturating_sub(CONTEXT_LINES);
        let end = (line0 + CONTEXT_LINES + 1).min(rows.len());
        let clip = |s: &str| -> String {
            if s.chars().count() <= MAX_CONTEXT_LINE_CHARS {
                s.to_owned()
            } else {
                let c: String = s.chars().take(MAX_CONTEXT_LINE_CHARS).collect();
                format!("{c}…")
            }
        };
        Some((
            rows[start..line0].iter().map(|s| clip(s)).collect(),
            clip(rows[line0]),
            rows[(line0 + 1)..end].iter().map(|s| clip(s)).collect(),
        ))
    }
}

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
    if release.is_empty() {
        return 0;
    }
    let Some(stack) = payload
        .get_mut("error")
        .and_then(|e| e.get_mut("stack"))
        .and_then(Value::as_array_mut)
    else {
        return 0;
    };

    let srcbundle = artifact_bytes(pool, attachments, project_id, release, "srcbundle", None)
        .await
        .and_then(|b| SrcBundle::parse(&b));

    let mut resolved = 0usize;
    match platform {
        "android" => {
            let Some(mapping) =
                artifact_bytes(pool, attachments, project_id, release, "proguard", None)
                    .await
                    .and_then(|b| ParsedMapping::parse(b).ok())
            else {
                return 0;
            };
            for frame in stack.iter_mut() {
                if rewrite_android(&mapping, frame) {
                    resolved += 1;
                }
                fill_context(srcbundle.as_ref(), frame);
            }
        }
        "ios" => {
            // Per-frame dSYM slice lookup, cached per event by uuid.
            let mut modules: HashMap<String, Option<Arc<DwarfModule>>> = HashMap::new();
            for frame in stack.iter_mut() {
                let Some(uuid) = frame
                    .get("imageUuid")
                    .and_then(Value::as_str)
                    .map(normalize_uuid)
                else {
                    continue;
                };
                if !modules.contains_key(&uuid) {
                    let m =
                        artifact_bytes(pool, attachments, project_id, release, "dsym", Some(&uuid))
                            .await
                            .and_then(|b| DwarfModule::from_bytes(b).ok().map(Arc::new));
                    modules.insert(uuid.clone(), m);
                }
                if let Some(Some(module)) = modules.get(&uuid)
                    && rewrite_ios(module, frame)
                {
                    resolved += 1;
                }
                fill_context(srcbundle.as_ref(), frame);
            }
        }
        _ => {}
    }
    resolved
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
fn fill_context(bundle: Option<&SrcBundle>, frame: &mut Value) {
    let Some(bundle) = bundle else { return };
    let Some(obj) = frame.as_object_mut() else {
        return;
    };
    if obj.contains_key("preContext") {
        return;
    }
    let (Some(file), Some(line)) = (
        obj.get("file").and_then(Value::as_str).map(str::to_owned),
        obj.get("line")
            .and_then(Value::as_u64)
            .and_then(|l| u32::try_from(l).ok()),
    ) else {
        return;
    };
    let Some((pre, at, post)) = bundle.window(&file, line) else {
        return;
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
) -> Option<Vec<u8>> {
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
    .await
    .ok()?;
    let hash_hex = rows.iter().find_map(|r| {
        let name: String = r.get("name");
        match name_uuid {
            Some(u) => normalize_uuid(&name)
                .contains(u)
                .then(|| r.get::<String, _>("content_hash")),
            None => Some(r.get::<String, _>("content_hash")),
        }
    })?;
    let hash = hash_hex.parse().ok()?;
    attachments.get(&hash).await.ok()
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
        let bundle = SrcBundle::parse(json!({ "app/src/Pay.kt": src }).to_string().as_bytes())
            .ok_or("bundle parse failed")?;
        let mut frame = json!({ "file": "Pay.kt", "line": 15 });
        fill_context(Some(&bundle), &mut frame);
        assert_eq!(frame["contextLine"], "kt line 15");
        assert_eq!(
            frame["preContext"].as_array().ok_or("pre missing")?.len(),
            CONTEXT_LINES
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
