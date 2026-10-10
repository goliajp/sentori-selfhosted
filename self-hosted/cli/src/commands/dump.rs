//! Complete encrypted disaster-recovery snapshot, including referenced blobs.
use anyhow::Context;
use sentori_snapshot::RecoveryKey;
use sqlx::PgPool;
use std::{
    io::{Read, Write},
    path::Path,
};

pub fn read_key(path: &Path) -> anyhow::Result<RecoveryKey> {
    let file = std::fs::File::open(path).context("open operator-held recovery key file")?;
    let mut text = String::new();
    file.take(1024).read_to_string(&mut text)?;
    anyhow::ensure!(text.len() < 1024, "recovery key file is too large");
    sentori_snapshot::parse_key(&text)
}

pub fn fingerprint() -> anyhow::Result<Option<String>> {
    sentori_snapshot::master_key_fingerprint(
        crate::secret_env::read("SENTORI_MASTER_KEY")?.as_deref(),
    )
}

pub fn keygen(path: &Path) -> anyhow::Result<()> {
    let key =
        RecoveryKey::generate().map_err(|_| anyhow::anyhow!("system random source unavailable"))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .context("create a new private recovery key file")?;
    writeln!(file, "{}", hex::encode(key.as_bytes()))?;
    file.sync_all()?;
    Ok(())
}

pub async fn run(
    pool: &PgPool,
    out: &Path,
    blob_dir: &Path,
    key_file: &Path,
    quiet: bool,
) -> anyhow::Result<()> {
    let key = read_key(key_file)?;
    let manifest = sentori_snapshot::capture::capture(
        pool,
        blob_dir,
        out,
        key,
        include_str!("../../../../VERSION").trim(),
        crate::secret_env::read("SENTORI_MASTER_KEY")?
            .map(|s| sentori_snapshot::parse_key(&s))
            .transpose()?,
    )
    .await?;
    if !quiet {
        println!("{}", serde_json::to_string_pretty(&manifest)?);
    }
    Ok(())
}
