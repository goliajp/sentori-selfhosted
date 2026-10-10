//! Structural preflight: exact tables, column shapes, transcript and content hashes.
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use anyhow::{Context, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use sentori_secrets_vault::MasterKey;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::TABLES;
use crate::archive::{ArchiveReader, CHUNK_SIZE, Column, Header, MAX_ARCHIVE, Mode, Record};

#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    pub header: Header,
    pub tables: BTreeMap<String, TableSummary>,
    pub blobs: BTreeMap<String, u64>,
    pub blob_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableSummary {
    pub columns: Vec<Column>,
    pub rows: u64,
    pub sha256: String,
}

pub fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// Read cancellation is checked for every bounded encrypted frame, including
/// when independent verification runs outside a capture operation.
pub fn verify_controlled<R: Read>(
    source: R,
    key: MasterKey,
    control: &crate::capture::CaptureControl,
) -> anyhow::Result<Manifest> {
    struct CheckedReader<'a, R> {
        inner: R,
        control: &'a crate::capture::CaptureControl,
    }
    impl<R: Read> Read for CheckedReader<'_, R> {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            self.control.check().map_err(std::io::Error::other)?;
            self.inner.read(bytes)
        }
    }
    verify(
        CheckedReader {
            inner: source,
            control,
        },
        key,
    )
}

pub fn verify<R: Read>(source: R, key: MasterKey) -> anyhow::Result<Manifest> {
    let mut reader = ArchiveReader::new(source, key)?;
    let Some(Record::Header { header }) = reader.read_record()? else {
        anyhow::bail!("missing archive header");
    };
    let tables = if header.mode == Mode::ProjectMigration {
        crate::migration::TABLES
    } else {
        TABLES
    };
    let mut scope = if header.mode == Mode::ProjectMigration {
        if let Some(binding) = &header.transfer {
            ensure!(
                !binding.id.is_nil()
                    && !binding.target_instance_id.is_nil()
                    && binding.target_instance_id != header.instance_id
                    && binding.target_generation > 0,
                "invalid transfer destination binding"
            );
        }
        ensure!(
            header.master_key_fingerprint.is_none(),
            "migration package carries instance key metadata"
        );
        Some(crate::migration::Scope::new(&header.project_ids)?)
    } else {
        ensure!(
            header.transfer.is_none(),
            "recovery archive cannot carry a transfer binding"
        );
        ensure!(
            header.project_ids.is_empty(),
            "recovery archive cannot have a project filter"
        );
        None
    };
    ensure!(
        header.migrations == crate::expected_migrations(),
        "archive schema is incompatible with this binary"
    );
    let mut manifest = Manifest {
        header,
        tables: BTreeMap::new(),
        blobs: BTreeMap::new(),
        blob_bytes: 0,
    };
    let mut references = BTreeSet::new();
    let mut reference_sizes = BTreeMap::new();
    let mut table: Option<(String, Vec<Column>, u64, Sha256)> = None;
    let mut blob: Option<(String, u64, u64, Sha256)> = None;
    let mut table_index = 0;
    while let Some(record) = reader.read_record()? {
        match record {
            Record::Header { .. } | Record::End { .. } => {
                anyhow::bail!("unexpected archive boundary")
            }
            Record::Table { name, columns } => {
                ensure!(
                    table.is_none() && blob.is_none() && manifest.blobs.is_empty(),
                    "interleaved archive entries"
                );
                ensure!(
                    tables
                        .get(table_index)
                        .is_some_and(|expected| *expected == name),
                    "missing, reordered or unknown table"
                );
                ensure!(
                    !columns.is_empty() && columns.len() <= 256,
                    "invalid column list"
                );
                ensure!(
                    columns
                        .iter()
                        .map(|c| &c.name)
                        .collect::<BTreeSet<_>>()
                        .len()
                        == columns.len(),
                    "duplicate column"
                );
                ensure!(
                    columns.iter().all(|c| !c.name.is_empty()
                        && c.name.len() <= 63
                        && c.name
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')),
                    "invalid column name"
                );
                if scope.is_some() {
                    let allowed = crate::migration::columns(&name);
                    ensure!(
                        columns.len() == allowed.len()
                            && columns.iter().all(|c| allowed.contains(&c.name.as_str())),
                        "migration columns exceed the positive allowlist"
                    );
                }
                table = Some((name, columns, 0, Sha256::new()));
            }
            Record::Row { table: name, value } => {
                let (active, columns, count, digest) =
                    table.as_mut().context("row outside a table")?;
                ensure!(*active == name, "row belongs to another table");
                let object = value.as_object().context("row must be an object")?;
                ensure!(
                    object.len() == columns.len()
                        && columns.iter().all(|c| object.contains_key(&c.name)),
                    "row columns differ from manifest"
                );
                if let Some(scope) = &mut scope {
                    scope.row(&name, &value)?;
                }
                if name == "instance_settings" {
                    ensure!(
                        *count == 0
                            && value["id"].as_str()
                                == Some(manifest.header.instance_id.to_string().as_str())
                            && value["generation"].as_i64() == Some(manifest.header.generation),
                        "snapshot identity mismatch"
                    );
                }
                let hash_column = match name.as_str() {
                    "event_attachments" => Some("blob_hash"),
                    "release_artifacts" => Some("content_hash"),
                    _ => None,
                };
                if let Some(column) = hash_column {
                    let hash = value[column].as_str().context("missing blob reference")?;
                    ensure!(valid_hash(hash), "invalid blob reference");
                    references.insert(hash.to_owned());
                    if !value["size_bytes"].is_null() {
                        let size = value["size_bytes"]
                            .as_u64()
                            .context("invalid referenced blob size")?;
                        if let Some(previous) = reference_sizes.insert(hash.to_owned(), size) {
                            ensure!(previous == size, "inconsistent sizes for the same blob");
                        }
                    }
                }
                let bytes = serde_json::to_vec(&value)?;
                digest.update(&bytes);
                digest.update(b"\n");
                *count += 1;
            }
            Record::TableEnd { name, rows, sha256 } => {
                let (active, columns, count, digest) =
                    table.take().context("table footer without table")?;
                ensure!(
                    active == name && count == rows && hex::encode(digest.finalize()) == sha256,
                    "table count or checksum mismatch"
                );
                ensure!(
                    name != "instance_settings" || rows == 1,
                    "instance identity missing"
                );
                manifest.tables.insert(
                    name,
                    TableSummary {
                        columns,
                        rows,
                        sha256,
                    },
                );
                table_index += 1;
            }
            Record::Blob { hash, bytes } => {
                ensure!(
                    table_index == tables.len() && table.is_none() && blob.is_none(),
                    "blob before complete tables or nested blob"
                );
                ensure!(
                    valid_hash(&hash)
                        && references.contains(&hash)
                        && !manifest.blobs.contains_key(&hash),
                    "unreferenced, duplicate or invalid blob"
                );
                ensure!(bytes <= MAX_ARCHIVE, "blob size limit exceeded");
                ensure!(
                    reference_sizes
                        .get(&hash)
                        .is_none_or(|expected| *expected == bytes),
                    "blob size differs from its reference"
                );
                blob = Some((hash, bytes, 0, Sha256::new()));
            }
            Record::Chunk {
                hash,
                offset,
                base64,
            } => {
                let (active, size, position, digest) =
                    blob.as_mut().context("chunk outside blob")?;
                ensure!(
                    *active == hash && offset == *position,
                    "blob chunk identity or offset mismatch"
                );
                ensure!(
                    base64.len() <= CHUNK_SIZE.div_ceil(3) * 4,
                    "blob chunk too large"
                );
                let bytes = STANDARD.decode(base64).context("invalid blob encoding")?;
                ensure!(
                    !bytes.is_empty() && bytes.len() <= CHUNK_SIZE,
                    "invalid blob chunk size"
                );
                *position = position
                    .checked_add(bytes.len() as u64)
                    .context("blob size overflow")?;
                ensure!(*position <= *size, "blob longer than declared");
                digest.update(&bytes);
            }
            Record::BlobEnd { hash } => {
                let (active, size, position, digest) =
                    blob.take().context("blob footer without blob")?;
                ensure!(
                    active == hash && position == size && hex::encode(digest.finalize()) == hash,
                    "blob checksum or size mismatch"
                );
                manifest.blob_bytes = manifest
                    .blob_bytes
                    .checked_add(size)
                    .context("blob total overflow")?;
                ensure!(
                    manifest.blob_bytes <= MAX_ARCHIVE,
                    "blob total exceeds limit"
                );
                manifest.blobs.insert(hash, size);
            }
        }
    }
    ensure!(
        table.is_none() && blob.is_none() && table_index == tables.len(),
        "incomplete snapshot"
    );
    ensure!(
        references.len() == manifest.blobs.len()
            && references
                .iter()
                .all(|hash| manifest.blobs.contains_key(hash)),
        "referenced blobs missing"
    );
    if let Some(scope) = scope {
        scope.finish()?;
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::ArchiveWriter;
    use serde_json::json;
    use uuid::Uuid;

    fn fixture(
        missing_table: bool,
        missing_blob: bool,
        corrupt_blob: bool,
    ) -> anyhow::Result<Vec<u8>> {
        let id = Uuid::new_v4();
        let bytes = b"recorded replay bytes";
        let hash = hex::encode(Sha256::digest(bytes));
        let header = Header {
            format: 1,
            archive_id: Uuid::new_v4(),
            instance_id: id,
            generation: 1,
            created_at: "2026-10-10T00:00:00Z".into(),
            application_version: "5.0.0".into(),
            migrations: crate::expected_migrations(),
            master_key_fingerprint: None,
            mode: Mode::DisasterRecovery,
            project_ids: Vec::new(),
            transfer: None,
        };
        let mut out = ArchiveWriter::new(Vec::new(), MasterKey::from_bytes([7; 32]), header)?;
        for name in TABLES {
            if missing_table && *name == "backend_checks" {
                continue;
            }
            let row = match *name {
                "instance_settings" => Some(json!({"id":id,"generation":1})),
                "event_attachments" => Some(json!({"blob_hash":hash})),
                _ => None,
            };
            let columns = row
                .as_ref()
                .and_then(|v| v.as_object())
                .map_or_else(|| vec!["id".to_owned()], |o| o.keys().cloned().collect())
                .into_iter()
                .map(|name| Column {
                    name,
                    data_type: "text".into(),
                    nullable: true,
                    default_expression: None,
                })
                .collect();
            out.write(Record::Table {
                name: (*name).to_owned(),
                columns,
            })?;
            let mut digest = Sha256::new();
            let count = u64::from(row.is_some());
            if let Some(row) = row {
                digest.update(serde_json::to_vec(&row)?);
                digest.update(b"\n");
                out.write(Record::Row {
                    table: (*name).to_owned(),
                    value: row,
                })?;
            }
            out.write(Record::TableEnd {
                name: (*name).to_owned(),
                rows: count,
                sha256: hex::encode(digest.finalize()),
            })?;
        }
        if !missing_blob {
            out.write(Record::Blob {
                hash: hash.clone(),
                bytes: bytes.len() as u64,
            })?;
            out.write(Record::Chunk {
                hash: hash.clone(),
                offset: 0,
                base64: STANDARD.encode(if corrupt_blob {
                    b"modified replay bytes"
                } else {
                    bytes
                }),
            })?;
            out.write(Record::BlobEnd { hash })?;
        }
        out.finish()
    }
    #[test]
    fn complete_manifest_and_referenced_blobs_are_required() -> anyhow::Result<()> {
        let bytes = fixture(false, false, false)?;
        let manifest = verify(bytes.as_slice(), MasterKey::from_bytes([7; 32]))?;
        assert_eq!(manifest.tables.len(), TABLES.len());
        assert_eq!(manifest.blobs.len(), 1);
        for flags in [
            (true, true, false),
            (false, true, false),
            (false, false, true),
        ] {
            let bytes = fixture(flags.0, flags.1, flags.2)?;
            assert!(verify(bytes.as_slice(), MasterKey::from_bytes([7; 32])).is_err());
        }
        Ok(())
    }
    #[test]
    fn paths_are_never_blob_identifiers() {
        assert!(valid_hash(&"ab".repeat(32)));
        for value in [
            "../secrets",
            "/etc/passwd",
            "",
            &"AB".repeat(32),
            &"g0".repeat(32),
        ] {
            assert!(!valid_hash(value));
        }
    }
}
