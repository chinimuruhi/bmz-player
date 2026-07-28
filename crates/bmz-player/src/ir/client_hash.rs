use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
    sync::OnceLock,
};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

pub const UNKNOWN_CLIENT_HASH: &str = "UNKNOWN";

static CLIENT_HASH: OnceLock<String> = OnceLock::new();

/// rianIRへ送る実行ファイルのSHA-256。
///
/// 開発buildはallowlist運用の対象外なので、既存の開発用識別子へフォールバックする。
pub fn current_client_hash() -> &'static str {
    CLIENT_HASH
        .get_or_init(|| {
            if cfg!(debug_assertions) || std::env::var_os("FLATPAK_ID").is_some() {
                return UNKNOWN_CLIENT_HASH.to_string();
            }
            std::env::current_exe()
                .context("failed to resolve current executable")
                .and_then(|path| sha256_file(&path))
                .unwrap_or_else(|error| {
                    tracing::warn!(%error, "failed to compute rianIR client hash");
                    UNKNOWN_CLIENT_HASH.to_string()
                })
        })
        .as_str()
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let file = File::open(path)
        .with_context(|| format!("failed to open client executable: {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("failed to read client executable: {}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_file_bytes_as_lowercase_sha256() {
        let root = std::env::temp_dir().join(format!(
            "bmz-client-hash-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("bmz-player");
        std::fs::write(&path, b"bmz-player\n").unwrap();

        assert_eq!(
            sha256_file(&path).unwrap(),
            "762057aa6faddfbaa742af8462ce5d8a1aa6042260ad7cb5a3af411e32073e1f"
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}
