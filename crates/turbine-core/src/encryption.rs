//! Encryption support for Turbine
//!
//! Provides encryption at rest for task payloads and results:
//! - AES-256-GCM authenticated encryption
//! - Key derivation and rotation
//! - Secure key storage patterns
//!
//! # Security Features
//! - Authenticated encryption (AEAD) prevents tampering
//! - Unique nonces per encryption operation
//! - Key zeroization on drop
//! - No plaintext key logging

#[cfg(feature = "encryption")]
mod implementation {
    use aes_gcm::{
        aead::{Aead, KeyInit, OsRng},
        Aes256Gcm, Nonce,
    };
    use base64::Engine;
    use rand::RngCore;
    use serde::{Deserialize, Serialize};
    use zeroize::{Zeroize, ZeroizeOnDrop};

    /// Encryption configuration
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EncryptionConfig {
        /// Enable encryption
        pub enabled: bool,

        /// Key ID for key rotation support
        pub key_id: String,

        /// Encrypt task arguments
        pub encrypt_args: bool,

        /// Encrypt task results
        pub encrypt_results: bool,

        /// Encrypt task metadata
        pub encrypt_metadata: bool,
    }

    impl Default for EncryptionConfig {
        fn default() -> Self {
            Self {
                enabled: false,
                key_id: "default".to_string(),
                encrypt_args: true,
                encrypt_results: true,
                encrypt_metadata: false,
            }
        }
    }

    /// A cryptographic key that is zeroized on drop
    #[derive(Clone, Zeroize, ZeroizeOnDrop)]
    pub struct EncryptionKey {
        /// The raw key bytes (32 bytes for AES-256)
        key: [u8; 32],
    }

    impl EncryptionKey {
        /// Generate a new random encryption key
        pub fn generate() -> Self {
            let mut key = [0u8; 32];
            OsRng.fill_bytes(&mut key);
            Self { key }
        }

        /// Create a key from raw bytes
        pub fn from_bytes(bytes: &[u8]) -> Result<Self, EncryptionError> {
            if bytes.len() != 32 {
                return Err(EncryptionError::InvalidKeyLength {
                    expected: 32,
                    actual: bytes.len(),
                });
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(bytes);
            Ok(Self { key })
        }

        /// Create a key from base64-encoded string
        pub fn from_base64(encoded: &str) -> Result<Self, EncryptionError> {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| EncryptionError::InvalidKey(e.to_string()))?;
            Self::from_bytes(&bytes)
        }

        /// Export key as base64 (use with caution)
        pub fn to_base64(&self) -> String {
            base64::engine::general_purpose::STANDARD.encode(&self.key)
        }

        /// Get the raw key bytes (use with caution)
        pub fn as_bytes(&self) -> &[u8; 32] {
            &self.key
        }
    }

    impl std::fmt::Debug for EncryptionKey {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // Never print the actual key
            f.debug_struct("EncryptionKey")
                .field("key", &"[REDACTED]")
                .finish()
        }
    }

    /// Encrypted data with metadata for decryption
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EncryptedData {
        /// Key ID used for encryption (for key rotation)
        pub key_id: String,

        /// Nonce used for encryption (12 bytes, base64 encoded)
        pub nonce: String,

        /// Encrypted ciphertext (base64 encoded)
        pub ciphertext: String,

        /// Version of the encryption format
        pub version: u8,
    }

    impl EncryptedData {
        /// Serialize to JSON for storage
        pub fn to_json(&self) -> Result<String, EncryptionError> {
            serde_json::to_string(self).map_err(|e| EncryptionError::SerializationError(e.to_string()))
        }

        /// Deserialize from JSON
        pub fn from_json(json: &str) -> Result<Self, EncryptionError> {
            serde_json::from_str(json).map_err(|e| EncryptionError::DeserializationError(e.to_string()))
        }

        /// Serialize to bytes for storage
        pub fn to_bytes(&self) -> Result<Vec<u8>, EncryptionError> {
            serde_json::to_vec(self).map_err(|e| EncryptionError::SerializationError(e.to_string()))
        }

        /// Deserialize from bytes
        pub fn from_bytes(bytes: &[u8]) -> Result<Self, EncryptionError> {
            serde_json::from_slice(bytes).map_err(|e| EncryptionError::DeserializationError(e.to_string()))
        }
    }

    /// Encryptor for encrypting and decrypting data
    pub struct Encryptor {
        config: EncryptionConfig,
        cipher: Aes256Gcm,
        key_id: String,
    }

    impl Encryptor {
        /// Create a new encryptor with the given key
        pub fn new(key: &EncryptionKey, config: EncryptionConfig) -> Self {
            let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
                .expect("Key length already validated");
            let key_id = config.key_id.clone();
            Self {
                config,
                cipher,
                key_id,
            }
        }

        /// Check if encryption is enabled
        pub fn is_enabled(&self) -> bool {
            self.config.enabled
        }

        /// Encrypt plaintext data
        pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, EncryptionError> {
            if !self.config.enabled {
                return Err(EncryptionError::Disabled);
            }

            // Generate random nonce (12 bytes for AES-GCM)
            let mut nonce_bytes = [0u8; 12];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = Nonce::from_slice(&nonce_bytes);

            // Encrypt
            let ciphertext = self
                .cipher
                .encrypt(nonce, plaintext)
                .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

            Ok(EncryptedData {
                key_id: self.key_id.clone(),
                nonce: base64::engine::general_purpose::STANDARD.encode(nonce_bytes),
                ciphertext: base64::engine::general_purpose::STANDARD.encode(ciphertext),
                version: 1,
            })
        }

        /// Decrypt encrypted data
        pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>, EncryptionError> {
            if !self.config.enabled {
                return Err(EncryptionError::Disabled);
            }

            // Verify key ID matches
            if encrypted.key_id != self.key_id {
                return Err(EncryptionError::KeyMismatch {
                    expected: self.key_id.clone(),
                    actual: encrypted.key_id.clone(),
                });
            }

            // Decode nonce
            let nonce_bytes = base64::engine::general_purpose::STANDARD
                .decode(&encrypted.nonce)
                .map_err(|e| EncryptionError::InvalidNonce(e.to_string()))?;

            if nonce_bytes.len() != 12 {
                return Err(EncryptionError::InvalidNonce(format!(
                    "Expected 12 bytes, got {}",
                    nonce_bytes.len()
                )));
            }

            let nonce = Nonce::from_slice(&nonce_bytes);

            // Decode ciphertext
            let ciphertext = base64::engine::general_purpose::STANDARD
                .decode(&encrypted.ciphertext)
                .map_err(|e| EncryptionError::InvalidCiphertext(e.to_string()))?;

            // Decrypt
            let plaintext = self
                .cipher
                .decrypt(nonce, ciphertext.as_ref())
                .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;

            Ok(plaintext)
        }

        /// Encrypt a serializable value
        pub fn encrypt_value<T: Serialize>(&self, value: &T) -> Result<EncryptedData, EncryptionError> {
            let json = serde_json::to_vec(value)
                .map_err(|e| EncryptionError::SerializationError(e.to_string()))?;
            self.encrypt(&json)
        }

        /// Decrypt to a deserializable value
        pub fn decrypt_value<T: for<'de> Deserialize<'de>>(
            &self,
            encrypted: &EncryptedData,
        ) -> Result<T, EncryptionError> {
            let plaintext = self.decrypt(encrypted)?;
            serde_json::from_slice(&plaintext)
                .map_err(|e| EncryptionError::DeserializationError(e.to_string()))
        }
    }

    impl std::fmt::Debug for Encryptor {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Encryptor")
                .field("config", &self.config)
                .field("key_id", &self.key_id)
                .field("cipher", &"[AES-256-GCM]")
                .finish()
        }
    }

    /// Key manager for handling multiple keys and rotation
    pub struct KeyManager {
        /// Current active key ID
        active_key_id: String,

        /// All available keys (key_id -> key)
        keys: std::collections::HashMap<String, EncryptionKey>,
    }

    impl KeyManager {
        /// Create a new key manager with a single key
        pub fn new(key_id: impl Into<String>, key: EncryptionKey) -> Self {
            let key_id = key_id.into();
            let mut keys = std::collections::HashMap::new();
            keys.insert(key_id.clone(), key);
            Self {
                active_key_id: key_id,
                keys,
            }
        }

        /// Add a key for decryption (old keys for rotation)
        pub fn add_key(&mut self, key_id: impl Into<String>, key: EncryptionKey) {
            self.keys.insert(key_id.into(), key);
        }

        /// Rotate to a new key
        pub fn rotate_key(&mut self, new_key_id: impl Into<String>, new_key: EncryptionKey) {
            let new_key_id = new_key_id.into();
            self.keys.insert(new_key_id.clone(), new_key);
            self.active_key_id = new_key_id;
        }

        /// Get the active key ID
        pub fn active_key_id(&self) -> &str {
            &self.active_key_id
        }

        /// Get a key by ID
        pub fn get_key(&self, key_id: &str) -> Option<&EncryptionKey> {
            self.keys.get(key_id)
        }

        /// Get the active key
        pub fn active_key(&self) -> &EncryptionKey {
            self.keys.get(&self.active_key_id).expect("Active key must exist")
        }

        /// Create an encryptor for the active key
        pub fn encryptor(&self, config: EncryptionConfig) -> Encryptor {
            let mut config = config;
            config.key_id = self.active_key_id.clone();
            Encryptor::new(self.active_key(), config)
        }

        /// Decrypt data using the appropriate key based on key_id in the encrypted data
        pub fn decrypt(&self, encrypted: &EncryptedData, config: &EncryptionConfig) -> Result<Vec<u8>, EncryptionError> {
            let key = self
                .get_key(&encrypted.key_id)
                .ok_or_else(|| EncryptionError::KeyNotFound(encrypted.key_id.clone()))?;

            let mut cfg = config.clone();
            cfg.key_id = encrypted.key_id.clone();
            let encryptor = Encryptor::new(key, cfg);
            encryptor.decrypt(encrypted)
        }
    }

    impl std::fmt::Debug for KeyManager {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("KeyManager")
                .field("active_key_id", &self.active_key_id)
                .field("key_count", &self.keys.len())
                .finish()
        }
    }

    /// Encryption errors
    #[derive(Debug, thiserror::Error)]
    pub enum EncryptionError {
        #[error("Encryption is disabled")]
        Disabled,

        #[error("Invalid key length: expected {expected}, got {actual}")]
        InvalidKeyLength { expected: usize, actual: usize },

        #[error("Invalid key: {0}")]
        InvalidKey(String),

        #[error("Key not found: {0}")]
        KeyNotFound(String),

        #[error("Key mismatch: expected {expected}, got {actual}")]
        KeyMismatch { expected: String, actual: String },

        #[error("Encryption failed: {0}")]
        EncryptionFailed(String),

        #[error("Decryption failed: {0}")]
        DecryptionFailed(String),

        #[error("Invalid nonce: {0}")]
        InvalidNonce(String),

        #[error("Invalid ciphertext: {0}")]
        InvalidCiphertext(String),

        #[error("Serialization error: {0}")]
        SerializationError(String),

        #[error("Deserialization error: {0}")]
        DeserializationError(String),
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_key_generation() {
            let key = EncryptionKey::generate();
            assert_eq!(key.as_bytes().len(), 32);
        }

        #[test]
        fn test_key_from_base64() {
            let key = EncryptionKey::generate();
            let encoded = key.to_base64();
            let decoded = EncryptionKey::from_base64(&encoded).unwrap();
            assert_eq!(key.as_bytes(), decoded.as_bytes());
        }

        #[test]
        fn test_encrypt_decrypt() {
            let key = EncryptionKey::generate();
            let config = EncryptionConfig {
                enabled: true,
                ..Default::default()
            };
            let encryptor = Encryptor::new(&key, config);

            let plaintext = b"Hello, Turbine!";
            let encrypted = encryptor.encrypt(plaintext).unwrap();
            let decrypted = encryptor.decrypt(&encrypted).unwrap();

            assert_eq!(plaintext.as_slice(), decrypted.as_slice());
        }

        #[test]
        fn test_encrypt_decrypt_value() {
            let key = EncryptionKey::generate();
            let config = EncryptionConfig {
                enabled: true,
                ..Default::default()
            };
            let encryptor = Encryptor::new(&key, config);

            #[derive(Debug, Serialize, Deserialize, PartialEq)]
            struct TestData {
                name: String,
                value: i32,
            }

            let data = TestData {
                name: "test".to_string(),
                value: 42,
            };

            let encrypted = encryptor.encrypt_value(&data).unwrap();
            let decrypted: TestData = encryptor.decrypt_value(&encrypted).unwrap();

            assert_eq!(data, decrypted);
        }

        #[test]
        fn test_unique_nonces() {
            let key = EncryptionKey::generate();
            let config = EncryptionConfig {
                enabled: true,
                ..Default::default()
            };
            let encryptor = Encryptor::new(&key, config);

            let plaintext = b"same plaintext";
            let encrypted1 = encryptor.encrypt(plaintext).unwrap();
            let encrypted2 = encryptor.encrypt(plaintext).unwrap();

            // Nonces should be different
            assert_ne!(encrypted1.nonce, encrypted2.nonce);
            // Ciphertexts should be different (due to different nonces)
            assert_ne!(encrypted1.ciphertext, encrypted2.ciphertext);
        }

        #[test]
        fn test_key_rotation() {
            let key1 = EncryptionKey::generate();
            let key2 = EncryptionKey::generate();

            let mut manager = KeyManager::new("key-v1", key1);

            let config = EncryptionConfig {
                enabled: true,
                ..Default::default()
            };

            // Encrypt with key1
            let encryptor1 = manager.encryptor(config.clone());
            let encrypted = encryptor1.encrypt(b"secret data").unwrap();
            assert_eq!(encrypted.key_id, "key-v1");

            // Rotate to key2
            manager.rotate_key("key-v2", key2);

            // New encryptions use key2
            let encryptor2 = manager.encryptor(config.clone());
            let encrypted2 = encryptor2.encrypt(b"new secret").unwrap();
            assert_eq!(encrypted2.key_id, "key-v2");

            // Can still decrypt old data with key1
            let decrypted = manager.decrypt(&encrypted, &config).unwrap();
            assert_eq!(decrypted, b"secret data");

            // Can decrypt new data with key2
            let decrypted2 = manager.decrypt(&encrypted2, &config).unwrap();
            assert_eq!(decrypted2, b"new secret");
        }

        #[test]
        fn test_tamper_detection() {
            let key = EncryptionKey::generate();
            let config = EncryptionConfig {
                enabled: true,
                ..Default::default()
            };
            let encryptor = Encryptor::new(&key, config);

            let plaintext = b"sensitive data";
            let mut encrypted = encryptor.encrypt(plaintext).unwrap();

            // Tamper with ciphertext
            let mut ciphertext = base64::engine::general_purpose::STANDARD
                .decode(&encrypted.ciphertext)
                .unwrap();
            if !ciphertext.is_empty() {
                ciphertext[0] ^= 0xFF; // Flip bits
            }
            encrypted.ciphertext = base64::engine::general_purpose::STANDARD.encode(&ciphertext);

            // Decryption should fail
            let result = encryptor.decrypt(&encrypted);
            assert!(result.is_err());
        }

        #[test]
        fn test_disabled_encryption() {
            let key = EncryptionKey::generate();
            let config = EncryptionConfig {
                enabled: false,
                ..Default::default()
            };
            let encryptor = Encryptor::new(&key, config);

            let result = encryptor.encrypt(b"data");
            assert!(matches!(result, Err(EncryptionError::Disabled)));
        }
    }
}

#[cfg(feature = "encryption")]
pub use implementation::*;

// Stub types when encryption feature is disabled
#[cfg(not(feature = "encryption"))]
mod stub {
    use serde::{Deserialize, Serialize};

    /// Encryption configuration (stub when feature disabled)
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct EncryptionConfig {
        pub enabled: bool,
    }

    impl EncryptionConfig {
        pub fn disabled() -> Self {
            Self { enabled: false }
        }
    }
}

#[cfg(not(feature = "encryption"))]
pub use stub::*;
