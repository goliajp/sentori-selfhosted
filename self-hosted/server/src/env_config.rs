//! Env reading with Docker-secret `_FILE` variants.
//!
//! For every secret-bearing variable `X`, `X_FILE` names a file
//! whose trimmed contents stand in for the value (the standard
//! Docker/Kubernetes secret-mount convention). Direct env wins
//! when both are set — a compose override should beat a stale
//! mounted secret during debugging.

use tracing::warn;

/// `env(key)`, falling back to the contents of the file named by
/// `env(key + "_FILE")`. Empty/whitespace values count as unset.
#[must_use]
pub fn env_or_file(key: &str) -> Option<String> {
    let file_key = format!("{key}_FILE");
    resolve(
        std::env::var(key).ok(),
        std::env::var(&file_key).ok(),
        &file_key,
    )
}

/// The pure core: pick the direct value, else read the file.
fn resolve(direct: Option<String>, file_path: Option<String>, file_key: &str) -> Option<String> {
    if let Some(v) = non_empty(direct) {
        return Some(v);
    }
    let path = non_empty(file_path)?;
    match std::fs::read_to_string(&path) {
        Ok(contents) => non_empty(Some(contents)),
        Err(e) => {
            // Misconfigured secret mount: say so and behave as
            // unset — every consumer of these vars has a documented
            // degraded mode, and a hard crash here would take the
            // whole instance down over one optional secret.
            warn!(%file_key, %path, error = %e, "secret file unreadable — treating as unset");
            None
        }
    }
}

fn non_empty(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_env_wins_over_file() -> std::io::Result<()> {
        let f = std::env::temp_dir().join("sentori-envtest-direct");
        std::fs::write(&f, "from-file\n")?;
        let got = resolve(
            Some("direct".into()),
            Some(f.to_string_lossy().into_owned()),
            "T_FILE",
        );
        assert_eq!(got.as_deref(), Some("direct"));
        Ok(())
    }

    #[test]
    fn file_variant_is_read_and_trimmed() -> std::io::Result<()> {
        let f = std::env::temp_dir().join("sentori-envtest-file");
        std::fs::write(&f, "  s3cret\n")?;
        let got = resolve(None, Some(f.to_string_lossy().into_owned()), "T_FILE");
        assert_eq!(got.as_deref(), Some("s3cret"));
        Ok(())
    }

    #[test]
    fn empty_direct_falls_through_to_file() -> std::io::Result<()> {
        let f = std::env::temp_dir().join("sentori-envtest-empty");
        std::fs::write(&f, "value")?;
        let got = resolve(
            Some("  ".into()),
            Some(f.to_string_lossy().into_owned()),
            "T_FILE",
        );
        assert_eq!(got.as_deref(), Some("value"));
        Ok(())
    }

    #[test]
    fn missing_both_is_none() {
        assert_eq!(resolve(None, None, "T_FILE"), None);
    }

    #[test]
    fn unreadable_file_is_none() {
        let got = resolve(None, Some("/nonexistent/sentori-secret".into()), "T_FILE");
        assert_eq!(got, None);
    }
}
