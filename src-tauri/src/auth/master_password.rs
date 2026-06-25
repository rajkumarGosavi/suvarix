use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha256;
use hex;
use zeroize::Zeroize;

use crate::error::{AppError, Result};

const ITERATIONS: u32 = 100_000;
const SALT_LEN: usize = 32;
const KEY_LEN: usize = 32;

pub fn generate_salt() -> String {
    let mut salt = vec![0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    hex::encode(salt)
}

pub fn derive_key(password: &str, salt_hex: &str) -> Result<String> {
    let salt = hex::decode(salt_hex)
        .map_err(|_| AppError::Parse("invalid salt".into()))?;
    let mut key = vec![0u8; KEY_LEN];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, ITERATIONS, &mut key);
    let encoded = hex::encode(&key);
    key.zeroize();
    Ok(encoded)
}

pub fn hash_password(password: &str, salt_hex: &str) -> Result<String> {
    derive_key(password, salt_hex)
}

pub fn verify_password(password: &str, salt_hex: &str, stored_hash: &str) -> Result<bool> {
    let computed = hash_password(password, salt_hex)?;
    Ok(computed == stored_hash)
}
