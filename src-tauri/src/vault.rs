use aes_gcm::aead::rand_core::{OsRng, RngCore};
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MAGIC: &[u8; 8] = b"PVAULT01";
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

#[derive(Debug, Clone, serde::Serialize)]
pub struct VaultEncryptionStatus {
    pub enabled: bool,
    pub algorithm: String,
    pub key_path: String,
    pub encrypted_artifacts: bool,
    pub encrypted_exports: bool,
    pub integrity_layer: String,
}

#[derive(Debug, Clone)]
pub struct VaultCrypto {
    key_path: PathBuf,
}

impl VaultCrypto {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            key_path: app_data_dir.join("vault.key"),
        }
    }

    pub fn status(&self) -> VaultEncryptionStatus {
        VaultEncryptionStatus {
            enabled: true,
            algorithm: "AES-256-GCM".to_string(),
            key_path: self.key_path.to_string_lossy().into_owned(),
            encrypted_artifacts: true,
            encrypted_exports: true,
            integrity_layer: "SHA-256 plaintext digest before encryption".to_string(),
        }
    }

    pub fn is_encrypted_path(path: &Path) -> bool {
        path.extension().and_then(|ext| ext.to_str()) == Some("vault")
    }

    pub async fn encrypt_file(&self, source_path: &Path, target_path: &Path) -> Result<()> {
        let key = self.load_or_create_key().await?;
        let plaintext = fs::read(source_path)
            .await
            .with_context(|| format!("failed to read plaintext vault input {}", source_path.display()))?;

        let mut nonce_bytes = [0_u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let cipher = Aes256Gcm::new_from_slice(&key)?;
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: &plaintext,
                    aad: MAGIC,
                },
            )
            .map_err(|_| anyhow!("vault encryption failed"))?;

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut output = fs::File::create(target_path).await?;
        output.write_all(MAGIC).await?;
        output.write_all(&nonce_bytes).await?;
        output.write_all(&ciphertext).await?;
        output.flush().await?;
        Ok(())
    }

    pub async fn decrypt_file(&self, source_path: &Path, target_path: &Path) -> Result<()> {
        let plaintext = self.decrypt_to_bytes(source_path).await?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(target_path, plaintext).await?;
        Ok(())
    }

    pub async fn decrypt_to_bytes(&self, source_path: &Path) -> Result<Vec<u8>> {
        let mut input = fs::File::open(source_path)
            .await
            .with_context(|| format!("failed to open encrypted vault file {}", source_path.display()))?;
        let mut magic = [0_u8; 8];
        input.read_exact(&mut magic).await?;
        if &magic != MAGIC {
            return Err(anyhow!("vault file has invalid encryption header"));
        }
        let mut nonce_bytes = [0_u8; NONCE_LEN];
        input.read_exact(&mut nonce_bytes).await?;
        let mut ciphertext = Vec::new();
        input.read_to_end(&mut ciphertext).await?;

        let key = self.load_or_create_key().await?;
        let cipher = Aes256Gcm::new_from_slice(&key)?;
        cipher
            .decrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: &ciphertext,
                    aad: MAGIC,
                },
            )
            .map_err(|_| anyhow!("vault decryption failed"))
    }

    pub async fn sha256_plaintext(&self, source_path: &Path) -> Result<String> {
        let bytes = if Self::is_encrypted_path(source_path) {
            self.decrypt_to_bytes(source_path).await?
        } else {
            fs::read(source_path).await?
        };
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        Ok(hex::encode(hasher.finalize()))
    }

    async fn load_or_create_key(&self) -> Result<[u8; KEY_LEN]> {
        if self.key_path.exists() {
            let bytes = fs::read(&self.key_path).await?;
            if bytes.len() != KEY_LEN {
                return Err(anyhow!("vault key has invalid length"));
            }
            let mut key = [0_u8; KEY_LEN];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }

        if let Some(parent) = self.key_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let mut key = [0_u8; KEY_LEN];
        OsRng.fill_bytes(&mut key);
        fs::write(&self.key_path, key).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.key_path, std::fs::Permissions::from_mode(0o600));
        }

        Ok(key)
    }
}

pub fn decrypted_cache_path(app_data_dir: &Path, relative_path: &str) -> PathBuf {
    let mut sanitized = relative_path.replace('\\', "/");
    if let Some(stripped) = sanitized.strip_suffix(".vault") {
        sanitized = stripped.to_string();
    }
    app_data_dir.join("decrypted-cache").join(sanitized)
}
