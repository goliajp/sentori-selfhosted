//! Offline reset command arguments and operator-held key checks.
use sqlx::PgPool;
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct Args {
    #[arg(long)]
    permit_file: PathBuf,
    #[arg(long)]
    instance_id: uuid::Uuid,
    #[arg(long)]
    generation: i64,
    #[arg(long)]
    instance_name: String,
    #[arg(long)]
    blob_dir: PathBuf,
    #[arg(long)]
    backup: PathBuf,
    #[arg(long)]
    journal: PathBuf,
    #[arg(long)]
    key_file: PathBuf,
    /// Attest that backup and journal are on storage independent of this instance.
    #[arg(long)]
    confirm_independent_storage: bool,
}

pub async fn run(pool: &PgPool, args: &Args, quiet: bool) -> anyhow::Result<()> {
    anyhow::ensure!(
        args.confirm_independent_storage,
        "--confirm-independent-storage is required: backup and journal must survive loss of this instance"
    );
    let permit = super::dump::read_key(&args.permit_file)?;
    let key = super::dump::read_key(&args.key_file)?;
    let root = args.blob_dir.canonicalize()?;
    anyhow::ensure!(
        !args.key_file.canonicalize()?.starts_with(&root),
        "recovery key must be stored outside the instance blob directory"
    );
    let report = sentori_snapshot::reset::reset(
        pool,
        sentori_snapshot::reset::ResetRequest {
            permit: &hex::encode(permit.as_bytes()),
            instance_id: args.instance_id,
            generation: args.generation,
            instance_name: &args.instance_name,
            blob_root: &args.blob_dir,
            archive: &args.backup,
            journal: &args.journal,
            recovery_key: key,
            master_key: crate::secret_env::read("SENTORI_MASTER_KEY")?
                .map(|s| sentori_snapshot::parse_key(&s))
                .transpose()?,
            application_version: include_str!("../../../../VERSION").trim(),
        },
    )
    .await?;
    if !quiet {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }
    Ok(())
}
