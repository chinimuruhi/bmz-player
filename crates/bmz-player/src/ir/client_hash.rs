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

/// rianIRへ送るclient hash。
///
/// ローカル開発用の固定値がコンパイル時に設定されていればbuild profileにかかわらず
/// その値を使う。未設定のdebug buildは既存の開発用識別子へフォールバックし、release
/// buildは実行ファイルのSHA-256を使う。
pub fn current_client_hash() -> &'static str {
    CLIENT_HASH
        .get_or_init(|| {
            if let Some(hash) = option_env!("BMZ_RIANIR_DEV_CLIENT_HASH") {
                if is_lowercase_sha256_hex(hash) {
                    return hash.to_string();
                }
                tracing::warn!(
                    "invalid compile-time BMZ_RIANIR_DEV_CLIENT_HASH; expected 64 lowercase hex characters"
                );
                return UNKNOWN_CLIENT_HASH.to_string();
            }
            if cfg!(debug_assertions) {
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

fn is_lowercase_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
    fn accepts_only_lowercase_sha256_hex_for_development_client_hash() {
        assert!(is_lowercase_sha256_hex(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
        assert!(!is_lowercase_sha256_hex(
            "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF"
        ));
        assert!(!is_lowercase_sha256_hex(
            "g123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
        assert!(!is_lowercase_sha256_hex("0123456789abcdef"));
    }

    #[test]
    fn uses_compile_time_development_client_hash_when_configured() {
        let Some(hash) = option_env!("BMZ_RIANIR_DEV_CLIENT_HASH") else {
            return;
        };
        let expected = if is_lowercase_sha256_hex(hash) { hash } else { UNKNOWN_CLIENT_HASH };
        assert_eq!(current_client_hash(), expected);
    }

    #[test]
    fn hashes_file_bytes_as_lowercase_sha256() {
        let unique =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root =
            std::env::temp_dir().join(format!("bmz-client-hash-{}-{}", std::process::id(), unique));
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
