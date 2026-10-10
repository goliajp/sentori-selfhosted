//! Operator secrets support the same mounted-file convention as the server.
//! Unreadable configured files fail closed instead of silently changing keys.
use anyhow::{Context, ensure};
use std::io::Read;

pub fn read(name: &str) -> anyhow::Result<Option<String>> {
    if let Some(value) = std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(Some(value.trim().to_owned()));
    }
    let Some(path) = std::env::var(format!("{name}_FILE"))
        .ok()
        .filter(|path| !path.trim().is_empty())
    else {
        return Ok(None);
    };
    let mut contents = String::new();
    std::fs::File::open(path)
        .context("open configured secret file")?
        .take(16 * 1024)
        .read_to_string(&mut contents)?;
    ensure!(
        contents.len() < 16 * 1024,
        "configured secret file too large"
    );
    ensure!(
        !contents.trim().is_empty(),
        "configured secret file is empty"
    );
    Ok(Some(contents.trim().to_owned()))
}
