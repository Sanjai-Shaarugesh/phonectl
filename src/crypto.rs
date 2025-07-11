use aes_gcm::aead::Aead;
use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::{Aes256Gcm, KeyInit};
use base64::engine::general_purpose::STANDARD;
use rand::Rng;
use std::fs;
use crate::config::get_key_file_path;
use base64::Engine;

pub fn generate_or_get_key() -> [u8; 32] {
    let key_path = get_key_file_path();

    if key_path.exists() {
        let encoded = fs::read_to_string(&key_path).unwrap_or_default();
        if let Ok(decoded) = STANDARD.decode(encoded.trim()) {
            if decoded.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&decoded);
                return key;
            } else {
                eprintln!("⚠️ Invalid key length found. Regenerating secure key...");
            }
        } else {
            eprintln!("⚠️ Corrupted key data. Regenerating secure key...");
        }
    }

    let mut rng = rand::rng();
    let key: [u8; 32] = rng.random();
    let encoded = STANDARD.encode(key);
    fs::write(key_path, encoded).expect("Failed to write key file");
    key
}

pub fn encrypt_data(data: &str) -> String {
    let key = generate_or_get_key();
    let key_array = GenericArray::from_slice(&key);
    let cipher = Aes256Gcm::new(key_array);
    let mut rng = rand::rng();
    let nonce_bytes: [u8; 12] = rng.random();
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, data.as_bytes()).unwrap();
    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    STANDARD.encode(result)
}

pub fn decrypt_data(encrypted: &str) -> Result<String, String> {
    let encrypted = encrypted.trim();
    let key = generate_or_get_key();
    let key_array = GenericArray::from_slice(&key);
    let cipher = Aes256Gcm::new(key_array);

    let data = STANDARD.decode(encrypted)
        .map_err(|e| format!("Base64 decode error: {}", e))?;
    if data.len() < 12 {
        return Err("Invalid encrypted data".to_string());
    }

    let nonce = GenericArray::from_slice(&data[..12]);
    let ciphertext = &data[12..];

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption error: {}", e))?;
    String::from_utf8(plaintext).map_err(|e| format!("UTF8 error: {}", e))
}