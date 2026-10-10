//! Verified, atomic restore into a new offline target.
use sqlx::PgPool;
use std::{fs::File, io::BufReader, path::Path};

pub fn verify(from: &Path, key_file: &Path, quiet: bool) -> anyhow::Result<()> {
    let manifest = sentori_snapshot::verify::verify(
        BufReader::new(File::open(from)?),
        super::dump::read_key(key_file)?,
    )?;
    if !quiet {
        println!("{}", serde_json::to_string_pretty(&manifest)?);
    }
    Ok(())
}

pub async fn run(
    pool: &PgPool,
    from: &Path,
    blob_dir: &Path,
    key_file: &Path,
    overwrite: bool,
    quiet: bool,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !overwrite,
        "--overwrite is not supported: restore requires an empty offline target"
    );
    let fingerprint = super::dump::fingerprint()?;
    let manifest = sentori_snapshot::restore::restore(
        pool,
        from,
        blob_dir,
        super::dump::read_key(key_file)?,
        fingerprint.as_deref(),
    )
    .await?;
    if !quiet {
        println!("{}", serde_json::to_string_pretty(&manifest)?);
    }
    Ok(())
}
