use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine};

const NONCE_SIZE: usize = 12;

pub struct TokenCrypto {
    cipher: Aes256Gcm,
}

impl TokenCrypto {
    pub fn new(key: &str) -> Result<Self> {
        let key_bytes = hex::decode(key)
            .map_err(|_| anyhow!("Invalid encryption key format. Expected hex string."))?;

        if key_bytes.len() != 32 {
            return Err(anyhow!(
                "Encryption key must be 32 bytes (64 hex characters)"
            ));
        }

        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);

        Ok(Self { cipher })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        // Combine nonce and ciphertext
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);

        // Base64 encode for storage
        Ok(STANDARD.encode(&combined))
    }

    pub fn decrypt(&self, encrypted: &str) -> Result<String> {
        // Base64 decode
        let combined = STANDARD
            .decode(encrypted)
            .map_err(|e| anyhow!("Invalid base64: {}", e))?;

        if combined.len() < NONCE_SIZE {
            return Err(anyhow!("Invalid encrypted data"));
        }

        // Split nonce and ciphertext
        let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        String::from_utf8(plaintext).map_err(|e| anyhow!("Invalid UTF-8: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        // Generate a test key (32 bytes = 64 hex chars)
        let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let crypto = TokenCrypto::new(key).unwrap();

        let plaintext = "my_secret_token_12345";
        let encrypted = crypto.encrypt(plaintext).unwrap();

        // Encrypted should be different from plaintext
        assert_ne!(encrypted, plaintext);

        // Should decrypt back to original
        let decrypted = crypto.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_different_nonces() {
        let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let crypto = TokenCrypto::new(key).unwrap();

        let plaintext = "my_secret_token";
        let encrypted1 = crypto.encrypt(plaintext).unwrap();
        let encrypted2 = crypto.encrypt(plaintext).unwrap();

        // Same plaintext should produce different ciphertexts (different nonces)
        assert_ne!(encrypted1, encrypted2);

        // Both should decrypt to same plaintext
        assert_eq!(crypto.decrypt(&encrypted1).unwrap(), plaintext);
        assert_eq!(crypto.decrypt(&encrypted2).unwrap(), plaintext);
    }
}
