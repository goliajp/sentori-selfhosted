//! Bounded, in-memory source archives. ZIP uploads and legacy path-to-text JSON
//! use the same parser for validation and source lookup; nothing is extracted.
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};

use anyhow::{Context, bail, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const MAX_FILE_BYTES: usize = 512 * 1024;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_FILES: usize = 10_000;
const MANIFEST_NAME: &str = "sentori-source-manifest.json";
const CONTEXT_LINES: usize = 11;
const MAX_CONTEXT_LINE_CHARS: usize = 300;

#[derive(Deserialize)]
struct Manifest {
    version: u32,
    release: String,
    files: Vec<ManifestFile>,
}
#[derive(Deserialize)]
struct ManifestFile {
    path: String,
    bytes: usize,
    sha256: String,
}

pub struct SrcBundle {
    files: HashMap<String, String>,
    pub release: Option<String>,
}

fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && !path
            .chars()
            .any(|c| c.is_control() || c == '\\' || c == ':')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn manifest_release(
    manifest: Option<Manifest>,
    files: &HashMap<String, String>,
) -> anyhow::Result<Option<String>> {
    if let Some(manifest) = manifest {
        ensure!(
            manifest.version == 1 && !manifest.release.trim().is_empty(),
            "invalid source manifest version or release"
        );
        ensure!(
            manifest.files.len() == files.len(),
            "manifest file count differs from ZIP"
        );
        let mut seen = HashSet::new();
        for item in manifest.files {
            ensure!(seen.insert(item.path.clone()), "duplicate manifest path");
            let content = files
                .get(&item.path)
                .context("manifest file missing from ZIP")?;
            ensure!(
                item.bytes == content.len()
                    && item.sha256 == hex::encode(Sha256::digest(content.as_bytes())),
                "source checksum mismatch: {}",
                item.path
            );
        }
        Ok(Some(manifest.release))
    } else {
        Ok(None)
    }
}

impl SrcBundle {
    pub fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
        ensure!(
            bytes.len() <= MAX_TOTAL_BYTES + MAX_MANIFEST_BYTES,
            "source bundle exceeds 68 MiB"
        );
        let mut files = HashMap::new();
        let mut manifest = None;
        if bytes.starts_with(b"PK") {
            let mut archive =
                zip::ZipArchive::new(Cursor::new(bytes)).context("invalid source ZIP")?;
            ensure!(archive.len() <= MAX_FILES + 1, "too many ZIP entries");
            let mut seen = HashSet::new();
            let mut total = 0usize;
            for index in 0..archive.len() {
                let mut file = archive.by_index(index).context("unreadable ZIP entry")?;
                let name = file.name()?.into_owned();
                let path = name.trim_end_matches('/');
                ensure!(
                    safe_path(path) && seen.insert(path.to_owned()),
                    "unsafe or duplicate source path: {name}"
                );
                ensure!(
                    !file.is_symlink() && !file.encrypted(),
                    "links and encrypted sources are unsupported"
                );
                if file.is_dir() {
                    continue;
                }
                let limit = if name == MANIFEST_NAME {
                    MAX_MANIFEST_BYTES
                } else {
                    MAX_FILE_BYTES
                };
                ensure!(
                    file.size() <= u64::try_from(limit)?,
                    "source file exceeds limit: {name}"
                );
                let mut body = Vec::new();
                (&mut file)
                    .take(u64::try_from(limit + 1)?)
                    .read_to_end(&mut body)?;
                ensure!(
                    body.len() <= limit,
                    "expanded source file exceeds limit: {name}"
                );
                total += body.len();
                ensure!(
                    total <= MAX_TOTAL_BYTES + MAX_MANIFEST_BYTES,
                    "expanded source bundle exceeds limit"
                );
                if name == MANIFEST_NAME {
                    manifest = Some(
                        serde_json::from_slice::<Manifest>(&body)
                            .context("invalid source manifest")?,
                    );
                } else {
                    ensure!(!body.contains(&0), "source contains binary data: {name}");
                    files.insert(
                        name,
                        String::from_utf8(body).context("source is not UTF-8")?,
                    );
                }
            }
        } else {
            let value: serde_json::Value = serde_json::from_slice(bytes)
                .context("expected source ZIP or path-to-text JSON")?;
            let object = value
                .as_object()
                .context("source JSON must be a path-to-text object")?;
            for (path, value) in object {
                let content = value.as_str().context("source JSON values must be text")?;
                ensure!(safe_path(path), "unsafe source path: {path}");
                ensure!(
                    content.len() <= MAX_FILE_BYTES && !content.contains('\0'),
                    "invalid or oversized source: {path}"
                );
                files.insert(path.clone(), content.to_owned());
            }
        }
        ensure!(
            !files.is_empty() && files.len() <= MAX_FILES,
            "source bundle must contain 1–10000 files"
        );
        ensure!(
            files.values().map(String::len).sum::<usize>() <= MAX_TOTAL_BYTES,
            "source text exceeds 64 MiB"
        );
        let release = manifest_release(manifest, &files)?;
        Ok(Self { files, release })
    }

    pub fn validate_release(&self, release: &str) -> anyhow::Result<()> {
        if self
            .release
            .as_deref()
            .is_some_and(|value| value != release)
        {
            bail!("source ZIP release does not match upload release {release}");
        }
        Ok(())
    }

    /// Match complete path suffixes on component boundaries. A shared filename
    /// under conflicting directories is not a match. Equal matches are ambiguous.
    fn lookup(&self, path: &str) -> Option<&str> {
        let normalized = path.replace('\\', "/");
        if let Some(content) = self.files.get(&normalized) {
            return Some(content);
        }
        let mut best = None;
        let mut score = 0;
        let mut ambiguous = false;
        for (candidate, content) in &self.files {
            let common = candidate
                .rsplit('/')
                .zip(normalized.rsplit('/'))
                .take_while(|(a, b)| a == b)
                .count();
            let required = candidate
                .split('/')
                .count()
                .min(normalized.split('/').count());
            if common == 0 || common < required {
                continue;
            }
            if common > score {
                best = Some(content.as_str());
                score = common;
                ambiguous = false;
            } else if common > 0 && common == score {
                ambiguous = true;
            }
        }
        if ambiguous { None } else { best }
    }

    pub fn window(&self, path: &str, line: u32) -> Option<(Vec<String>, String, Vec<String>)> {
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
                format!(
                    "{}…",
                    s.chars().take(MAX_CONTEXT_LINE_CHARS).collect::<String>()
                )
            }
        };
        Some((
            rows[start..line0].iter().map(|s| clip(s)).collect(),
            clip(rows[line0]),
            rows[line0 + 1..end].iter().map(|s| clip(s)).collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn archive(entries: &[(&str, &[u8])]) -> anyhow::Result<Vec<u8>> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, body) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())?;
            zip.write_all(body)?;
        }
        Ok(zip.finish()?.into_inner())
    }
    #[test]
    fn zip_and_legacy_json_supply_identical_context() -> anyhow::Result<()> {
        for bytes in [
            archive(&[("Sources/Pay.swift", b"one\ntwo\nthree")])?,
            br#"{"Sources/Pay.swift":"one\ntwo\nthree"}"#.to_vec(),
        ] {
            let bundle = SrcBundle::parse(&bytes)?;
            assert_eq!(
                bundle.window("/build/Sources/Pay.swift", 2).map(|x| x.1),
                Some("two".to_owned())
            );
        }
        Ok(())
    }
    #[test]
    fn rejects_traversal_binary_empty_and_oversized_sources() -> anyhow::Result<()> {
        for entries in [vec![("../a.kt", &b"x"[..])], vec![("a.kt", &b"\0x"[..])]] {
            assert!(SrcBundle::parse(&archive(&entries)?).is_err());
        }
        assert!(SrcBundle::parse(b"{}").is_err());
        assert!(SrcBundle::parse(br#"{"a.kt":42}"#).is_err());
        assert!(SrcBundle::parse(&archive(&[("a.kt", &vec![b'x'; MAX_FILE_BYTES + 1])])?).is_err());
        Ok(())
    }
    #[test]
    fn ambiguous_basenames_do_not_show_unrelated_source() -> anyhow::Result<()> {
        let bundle = SrcBundle::parse(br#"{"a/Pay.kt":"a","b/Pay.kt":"b","NotPay.kt":"wrong"}"#)?;
        assert!(bundle.window("Pay.kt", 1).is_none());
        assert_eq!(
            bundle.window("/build/a/Pay.kt", 1).map(|x| x.1),
            Some("a".to_owned())
        );
        assert!(bundle.window("ay.kt", 1).is_none());
        Ok(())
    }
    #[test]
    fn conflicting_directories_do_not_match_by_basename() -> anyhow::Result<()> {
        let bundle = SrcBundle::parse(br#"{"src/index.js":"original"}"#)?;
        assert!(
            bundle
                .window("https://cdn.example/assets/index.js", 1)
                .is_none()
        );
        assert!(bundle.window("/build/assets/index.js", 1).is_none());
        assert_eq!(
            bundle
                .window("https://cdn.example/src/index.js", 1)
                .map(|x| x.1),
            Some("original".to_owned())
        );
        Ok(())
    }
    #[test]
    fn manifest_pins_release_and_source_bytes() -> anyhow::Result<()> {
        let manifest = serde_json::json!({"version":1,"release":"app@1+7","files":[{"path":"a.kt","bytes":1,"sha256":hex::encode(Sha256::digest(b"x"))}]}).to_string();
        let bytes = archive(&[(MANIFEST_NAME, manifest.as_bytes()), ("a.kt", b"x")])?;
        let bundle = SrcBundle::parse(&bytes)?;
        bundle.validate_release("app@1+7")?;
        assert!(bundle.validate_release("app@1+8").is_err());
        assert!(
            SrcBundle::parse(&archive(&[
                (MANIFEST_NAME, manifest.as_bytes()),
                ("a.kt", b"y")
            ])?)
            .is_err()
        );
        Ok(())
    }
}
