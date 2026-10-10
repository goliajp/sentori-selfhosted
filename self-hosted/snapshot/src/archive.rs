//! Streaming authenticated frames. Each frame binds the archive ID and monotonic
//! index; the final frame authenticates the complete plaintext transcript.
//! No paths or extraction instructions are accepted from an archive.
use std::io::{Read, Write};

use anyhow::{Context, ensure};
use sentori_secrets_vault::{KeyId, MasterKey, Vault};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const MAGIC: &[u8; 16] = b"SENTORI-BACKUP-1";
pub const MAX_FRAME: usize = 16 * 1024 * 1024;
pub const MAX_ARCHIVE: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_FRAMES: u64 = 10_000_000;
pub const CHUNK_SIZE: usize = 512 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Migration {
    pub version: i64,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub format: u32,
    pub archive_id: Uuid,
    pub instance_id: Uuid,
    pub generation: i64,
    pub created_at: String,
    pub application_version: String,
    pub migrations: Vec<Migration>,
    /// Hash of the instance master key, never the key itself. It must be
    /// provisioned separately on a disaster-recovery target.
    pub master_key_fingerprint: Option<String>,
    pub mode: Mode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<TransferBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TransferBinding {
    pub id: Uuid,
    pub target_instance_id: Uuid,
    pub target_generation: i64,
    pub phase: TransferPhase,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferPhase {
    Copy,
    Final,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    DisasterRecovery,
    ProjectMigration,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_expression: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Record {
    Header {
        header: Header,
    },
    Table {
        name: String,
        columns: Vec<Column>,
    },
    Row {
        table: String,
        value: Value,
    },
    TableEnd {
        name: String,
        rows: u64,
        sha256: String,
    },
    Blob {
        hash: String,
        bytes: u64,
    },
    Chunk {
        hash: String,
        offset: u64,
        base64: String,
    },
    BlobEnd {
        hash: String,
    },
    End {
        frames: u64,
        sha256: String,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    archive_id: Uuid,
    index: u64,
    record: Record,
}

pub struct ArchiveWriter<W> {
    writer: W,
    vault: Vault,
    id: Uuid,
    index: u64,
    bytes: u64,
    transcript: Sha256,
}

fn vault(key: MasterKey) -> anyhow::Result<Vault> {
    Ok(Vault::new(key, KeyId::new("sentori-backup-v1")?))
}

impl<W: Write> ArchiveWriter<W> {
    pub fn new(mut writer: W, key: MasterKey, header: Header) -> anyhow::Result<Self> {
        ensure!(header.format == 1, "unsupported archive version");
        writer.write_all(MAGIC)?;
        let mut archive = Self {
            writer,
            vault: vault(key)?,
            id: header.archive_id,
            index: 0,
            bytes: MAGIC.len() as u64,
            transcript: Sha256::new(),
        };
        archive.write(Record::Header { header })?;
        Ok(archive)
    }

    pub fn write(&mut self, record: Record) -> anyhow::Result<()> {
        ensure!(
            !matches!(record, Record::End { .. }),
            "use finish to authenticate the archive"
        );
        ensure!(
            self.index == 0 || !matches!(record, Record::Header { .. }),
            "duplicate archive header"
        );
        self.frame(record)
    }

    fn frame(&mut self, record: Record) -> anyhow::Result<()> {
        ensure!(self.index < MAX_FRAMES, "archive frame limit exceeded");
        let frame = Frame {
            archive_id: self.id,
            index: self.index,
            record,
        };
        let plaintext = serde_json::to_vec(&frame)?;
        ensure!(
            plaintext.len() < MAX_FRAME - 1024,
            "archive record exceeds limit"
        );
        let sealed = self.vault.seal(&plaintext)?;
        ensure!(sealed.len() <= MAX_FRAME, "encrypted frame exceeds limit");
        self.bytes = self
            .bytes
            .checked_add(4 + sealed.len() as u64)
            .context("archive size overflow")?;
        ensure!(self.bytes <= MAX_ARCHIVE, "archive exceeds size limit");
        self.writer
            .write_all(&u32::try_from(sealed.len())?.to_be_bytes())?;
        self.writer.write_all(&sealed)?;
        self.transcript
            .update((plaintext.len() as u64).to_be_bytes());
        self.transcript.update(&plaintext);
        self.index += 1;
        Ok(())
    }

    pub fn finish(mut self) -> anyhow::Result<W> {
        self.frame(Record::End {
            frames: self.index,
            sha256: hex::encode(self.transcript.clone().finalize()),
        })?;
        self.writer.flush()?;
        Ok(self.writer)
    }
}

pub struct ArchiveReader<R> {
    reader: R,
    vault: Vault,
    id: Option<Uuid>,
    index: u64,
    bytes: u64,
    transcript: Sha256,
    ended: bool,
}

impl<R: Read> ArchiveReader<R> {
    pub fn new(mut reader: R, key: MasterKey) -> anyhow::Result<Self> {
        let mut magic = [0; 16];
        reader
            .read_exact(&mut magic)
            .context("archive header missing")?;
        ensure!(&magic == MAGIC, "unsupported archive format");
        Ok(Self {
            reader,
            vault: vault(key)?,
            id: None,
            index: 0,
            bytes: 16,
            transcript: Sha256::new(),
            ended: false,
        })
    }

    /// Returns `None` only after a valid authenticated footer and physical EOF.
    pub fn read_record(&mut self) -> anyhow::Result<Option<Record>> {
        if self.ended {
            return Ok(None);
        }
        ensure!(self.index < MAX_FRAMES, "archive frame limit exceeded");
        let mut length = [0; 4];
        self.reader
            .read_exact(&mut length)
            .context("archive truncated before authenticated footer")?;
        let length = u32::from_be_bytes(length) as usize;
        ensure!((64..=MAX_FRAME).contains(&length), "invalid frame length");
        self.bytes = self
            .bytes
            .checked_add(4 + length as u64)
            .context("archive size overflow")?;
        ensure!(self.bytes <= MAX_ARCHIVE, "archive exceeds size limit");
        let mut sealed = vec![0; length];
        self.reader
            .read_exact(&mut sealed)
            .context("archive frame truncated")?;
        let plaintext = self
            .vault
            .open(&sealed)
            .context("archive authentication failed")?;
        let frame: Frame = serde_json::from_slice(&plaintext).context("invalid archive frame")?;
        ensure!(
            frame.index == self.index,
            "reordered or missing archive frame"
        );
        if let Some(id) = self.id {
            ensure!(id == frame.archive_id, "mixed archives");
        } else {
            let Record::Header { ref header } = frame.record else {
                anyhow::bail!("archive must begin with a header");
            };
            ensure!(
                header.format == 1 && header.archive_id == frame.archive_id,
                "invalid archive header"
            );
            ensure!(
                header.generation > 0 && !header.migrations.is_empty(),
                "invalid archive metadata"
            );
            self.id = Some(frame.archive_id);
        }
        if let Record::End { frames, ref sha256 } = frame.record {
            ensure!(
                frames == self.index && *sha256 == hex::encode(self.transcript.clone().finalize()),
                "archive transcript mismatch"
            );
            let mut extra = [0];
            ensure!(
                self.reader.read(&mut extra)? == 0,
                "unexpected data after archive footer"
            );
            self.ended = true;
            return Ok(None);
        }
        ensure!(
            self.index == 0 || !matches!(frame.record, Record::Header { .. }),
            "duplicate archive header"
        );
        self.transcript
            .update((plaintext.len() as u64).to_be_bytes());
        self.transcript.update(&plaintext);
        self.index += 1;
        Ok(Some(frame.record))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn header() -> Header {
        Header {
            format: 1,
            archive_id: Uuid::new_v4(),
            instance_id: Uuid::new_v4(),
            generation: 1,
            created_at: "2026-10-10T00:00:00Z".into(),
            application_version: "5.0.0".into(),
            migrations: vec![Migration {
                version: 29,
                checksum: "fixture".into(),
            }],
            master_key_fingerprint: None,
            mode: Mode::DisasterRecovery,
            project_ids: Vec::new(),
            transfer: None,
        }
    }
    fn archive() -> anyhow::Result<Vec<u8>> {
        let mut writer = ArchiveWriter::new(Vec::new(), MasterKey::from_bytes([19; 32]), header())?;
        writer.write(Record::Row {
            table: "users".into(),
            value: serde_json::json!({"password_hash":"private fixture"}),
        })?;
        writer.finish()
    }
    fn read(bytes: &[u8], key: [u8; 32]) -> anyhow::Result<usize> {
        let mut reader = ArchiveReader::new(bytes, MasterKey::from_bytes(key))?;
        let mut count = 0;
        while reader.read_record()?.is_some() {
            count += 1;
        }
        Ok(count)
    }
    #[test]
    fn authenticated_roundtrip_and_wrong_key() -> anyhow::Result<()> {
        let bytes = archive()?;
        assert_eq!(read(&bytes, [19; 32])?, 2);
        assert!(read(&bytes, [20; 32]).is_err());
        assert!(!bytes.windows(15).any(|w| w == b"private fixture"));
        Ok(())
    }
    #[test]
    fn rejects_truncation_tampering_and_extra_bytes() -> anyhow::Result<()> {
        let bytes = archive()?;
        for n in [0, 15, 16, 20, bytes.len() / 2, bytes.len() - 1] {
            assert!(read(&bytes[..n], [19; 32]).is_err());
        }
        let mut tampered = bytes.clone();
        tampered[30] ^= 1;
        assert!(read(&tampered, [19; 32]).is_err());
        let mut appended = bytes;
        appended.push(0);
        assert!(read(&appended, [19; 32]).is_err());
        Ok(())
    }
    #[test]
    fn rejects_reorder_cross_archive_frames_and_length_bombs() -> anyhow::Result<()> {
        let bytes = archive()?;
        let first = 20 + u32::from_be_bytes(bytes[16..20].try_into()?) as usize;
        let next = first + 4 + u32::from_be_bytes(bytes[first..first + 4].try_into()?) as usize;
        let mut duplicate = bytes[..next].to_vec();
        duplicate.extend_from_slice(&bytes[first..]);
        assert!(read(&duplicate, [19; 32]).is_err());
        let other = archive()?;
        let offset = 20 + u32::from_be_bytes(other[16..20].try_into()?) as usize;
        let mut mixed = bytes[..first].to_vec();
        mixed.extend_from_slice(&other[offset..]);
        assert!(read(&mixed, [19; 32]).is_err());
        let mut bomb = bytes;
        bomb[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(read(&bomb, [19; 32]).is_err());
        Ok(())
    }
}
